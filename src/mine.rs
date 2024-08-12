use std::{sync::{Arc, RwLock}, time::Instant};
use threadpool::ThreadPool;
use colored::*;
use drillx::{equix::{self}, Hash, Solution};
use ore_api::{consts::{BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION}, state::{Bus, Config, Proof}};
use ore_utils::AccountDeserialize;
use rand::Rng;
use solana_program::pubkey::Pubkey;
use solana_rpc_client::spinner;
use solana_sdk::signer::Signer;
use crate::{args::MineArgs, send_and_confirm::ComputeBudget, utils::{amount_u64_to_string, get_clock, get_config, get_updated_proof_with_authority, proof_pubkey}, Miner};

impl Miner {
    pub async fn mine(&self, args: MineArgs) {
        // Open account, if needed.
        let signer = self.signer();
        self.open().await;

        // Check num threads
        self.check_num_cores(args.cores);

        // Create a thread pool with a number of threads equal to available cores
        let pool = ThreadPool::new(args.cores as usize);

        let mut last_hash_at = 0;
        let mut last_balance = 0;

        loop {
            // Start a 60-second timer
            let start_time = Instant::now();

            // Fetch proof
            let config = get_config(&self.rpc_client).await;
            let proof = get_updated_proof_with_authority(&self.rpc_client, signer.pubkey(), last_hash_at).await;

            println!(
                "\n\nStake: {} ORE\n{}  Multiplier: {:12}x",
                amount_u64_to_string(proof.balance),
                if last_hash_at.gt(&0) {
                    format!(
                        "  Change: {} ORE\n",
                        amount_u64_to_string(proof.balance.saturating_sub(last_balance))
                    )
                } else {
                    "".to_string()
                },
                calculate_multiplier(proof.balance, config.top_balance)
            );
            last_hash_at = proof.last_hash_at;
            last_balance = proof.balance;

            // Run drillx using the thread pool for the full 60 seconds
            let solution = self.find_hash_par(proof, args.cores, config.min_difficulty as u32, &pool).await;

            // After 60 seconds, submit the solution
            if start_time.elapsed().as_secs() >= 60 {
                // Build instruction set
                let mut ixs = vec![ore_api::instruction::auth(proof_pubkey(signer.pubkey()))];
                let mut compute_budget = 500_000;
                if self.should_reset(config).await && rand::thread_rng().gen_range(0..100).eq(&0) {
                    compute_budget += 100_000;
                    ixs.push(ore_api::instruction::reset(signer.pubkey()));
                }

                // Build mine ix
                ixs.push(ore_api::instruction::mine(
                    signer.pubkey(),
                    signer.pubkey(),
                    self.find_bus().await,
                    solution,
                ));

                // Submit transaction
                self.send_and_confirm(&ixs, ComputeBudget::Fixed(compute_budget), false)
                    .await
                    .ok();
            } else {
                println!("Mining period is not complete, continuing to mine...");
                continue;
            }
        }
    }

    async fn find_hash_par(
        &self,
        proof: Proof,
        cores: u64,
        min_difficulty: u32,
        pool: &ThreadPool,
    ) -> Solution {
        // Shared state across threads
        let progress_bar = Arc::new(spinner::new_progress_bar());
        let global_best_difficulty = Arc::new(RwLock::new(0u32));
        
        // Keep track of best result
        let best_result = Arc::new(RwLock::new((0u64, 0u32, Hash::default())));

        // Dispatch job to each thread in the pool
        let (tx, rx) = std::sync::mpsc::channel();

        for i in 0..cores {
            let global_best_difficulty = Arc::clone(&global_best_difficulty);
            let progress_bar = Arc::clone(&progress_bar);
            let best_result = Arc::clone(&best_result);
            let tx = tx.clone();
            let proof = proof.clone();

            pool.execute(move || {
                let timer = Instant::now();
                let mut nonce = u64::MAX.saturating_div(cores).saturating_mul(i);
                let mut memory = equix::SolverMemory::new();

                loop {
                    if let Ok(hx) = drillx::hash_with_memory(
                        &mut memory,
                        &proof.challenge,
                        &nonce.to_le_bytes(),
                    ) {
                        let difficulty = hx.difficulty();
                        {
                            let mut global_best = global_best_difficulty.write().unwrap();
                            if difficulty > *global_best {
                                *global_best = difficulty;
                                let mut best_result_lock = best_result.write().unwrap();
                                // Instead of cloning, we store the reference of hx in best_result_lock
                                *best_result_lock = (nonce, difficulty, hx);
                            }
                        }
                    }

                    // Increment nonce
                    nonce += 1;

                    // Exit if time has elapsed
                    if timer.elapsed().as_secs() >= 60 {
                        break;
                    }
                }

                // Notify the main thread that this worker has finished
                tx.send(()).unwrap();
            });
        }

        drop(tx); // Close the channel to signal the end of sending

        // Wait for all threads to finish
        for _ in 0..cores {
            rx.recv().unwrap();
        }

        // Get the best result
        let best_result_lock = best_result.read().unwrap();
        let (best_nonce, best_difficulty, best_hash) = (
            best_result_lock.0,
            best_result_lock.1,
            &best_result_lock.2, // Use a reference here instead of cloning
        );

        // Update progress bar
        progress_bar.finish_with_message(format!(
            "Best hash: {} (difficulty {})",
            bs58::encode(best_hash.h).into_string(),
            best_difficulty
        ));

        Solution::new(best_hash.d, best_nonce.to_le_bytes())
    }

    pub fn check_num_cores(&self, cores: u64) {
        let num_cores = num_cpus::get() as u64;
        if cores > num_cores {
            println!(
                "{} Cannot exceed available cores ({})",
                "WARNING".bold().yellow(),
                num_cores
            );
        }
    }

    pub async fn get_cutoff(&self, proof: Proof, buffer_time: u64) -> u64 {
        let clock = get_clock(&self.rpc_client).await;
        proof
            .last_hash_at
            .saturating_add(60)
            .saturating_sub(buffer_time as i64)
            .saturating_sub(clock.unix_timestamp)
            .max(0) as u64
    }

    pub async fn should_reset(&self, config: Config) -> bool {
        let clock = get_clock(&self.rpc_client).await;
        config
            .last_reset_at
            .saturating_add(EPOCH_DURATION)
            .saturating_sub(5) // Buffer
            .le(&clock.unix_timestamp)
    }

    pub async fn find_bus(&self) -> Pubkey {
        // Fetch the bus with the largest balance
        if let Ok(accounts) = self.rpc_client.get_multiple_accounts(&BUS_ADDRESSES).await {
            let mut top_bus_balance: u64 = 0;
            let mut top_bus = BUS_ADDRESSES[0];
            for account in accounts {
                if let Some(account) = account {
                    if let Ok(bus) = Bus::try_from_bytes(&account.data) {
                        if bus.rewards > top_bus_balance {
                            top_bus_balance = bus.rewards;
                            top_bus = BUS_ADDRESSES[bus.id as usize];
                        }
                    }
                }
            }
            return top_bus;
        }

        // Otherwise return a random bus
        let i = rand::thread_rng().gen_range(0..BUS_COUNT);
        BUS_ADDRESSES[i]
    }
}

fn calculate_multiplier(balance: u64, top_balance: u64) -> f64 {
    1.0 + (balance as f64 / top_balance as f64).min(1.0)
}
