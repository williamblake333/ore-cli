use std::io::{self, Write};
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use log::{info, error}; // Assuming the log crate is used
use ore::{self, state::Bus, BUS_ADDRESSES, BUS_COUNT, EPOCH_DURATION};
use rand::Rng;
use solana_program::{keccak::HASH_BYTES, program_memory::sol_memcmp, pubkey::Pubkey};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    keccak::{hashv, Hash as KeccakHash},
    signature::Signer,
};
use crate::{
    cu_limits::{CU_LIMIT_MINE, CU_LIMIT_RESET},
    utils::{get_clock_account, get_proof, get_treasury},
    Miner,
};

// Your existing code here...

impl Miner {
    pub async fn mine(&self, threads: u64) -> Result<(), Box<dyn std::error::Error>> {
        // Register, if needed. Consider handling errors from register.
        let signer = self.signer()?;
        self.register().await; // Consider what to do if registration fails
        let mut rng = rand::thread_rng();

        // Start mining loop
        loop {
            let balance = self.get_ore_display_balance().await?;
            let treasury = get_treasury(&self.rpc_client).await?;
            let proof = get_proof(&self.rpc_client, signer.pubkey()).await?;
            let rewards = (proof.claimable_rewards as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));
            let reward_rate = (treasury.reward_rate as f64) / (10f64.powf(ore::TOKEN_DECIMALS as f64));

            // Logging instead of directly writing to stdout
            info!("Balance: {} ORE", balance);
            info!("Claimable: {} ORE", rewards);
            info!("Reward rate: {} ORE", reward_rate);

            println!("\nMining for a valid hash..."); // Consider replacing with logging

            let (next_hash, nonce) = self.find_next_hash_par(proof.hash.into(), treasury.difficulty.into(), threads);

            // The rest of your mining logic here...
        }

        Ok(())
    }

    pub async fn get_ore_display_balance(&self) -> Result<String, Box<dyn std::error::Error>> {
        let client = self.rpc_client.clone();
        let signer = self.signer()?;
        let token_account_address = spl_associated_token_account::get_associated_token_address(
            &signer.pubkey(),
            &ore::MINT_ADDRESS,
        );
        match client.get_token_account(&token_account_address).await {
            Ok(token_account) => {
                if let Some(token_account) = token_account {
                    Ok(token_account.token_amount.ui_amount_string)
                } else {
                    Ok("0.00".to_string())
                }
            }
            Err(e) => {
                error!("Failed to get ORE display balance: {}", e);
                Err(e.into())
            },
        }
    }

    // Your existing methods...
    // Ensure other methods either handle errors or propagate them where applicable
}
