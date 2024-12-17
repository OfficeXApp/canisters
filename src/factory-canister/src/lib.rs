use candid::{CandidType, Encode, Principal};
use ic_cdk::api::management_canister::main::{
    create_canister, install_code, CanisterInstallMode, CreateCanisterArgument, InstallCodeArgument,
};
use ic_cdk::caller;
use ic_cdk_macros::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;

use regex::Regex;

type DriveCanisterId = Principal;

const DRIVE_WASM: &[u8] =
    include_bytes!("../../../target/wasm32-unknown-unknown/release/officex_canisters_backend.wasm");

#[derive(CandidType, Serialize, Deserialize, Clone)]
struct CanisterSettings {
    controllers: Option<Vec<Principal>>,
    compute_allocation: Option<u8>,
    memory_allocation: Option<u64>,
    freezing_threshold: Option<u64>,
}

#[derive(CandidType, Serialize, Deserialize)]
struct State {
    drives_counter: u64,
    user_drive_directory: HashMap<Principal, DriveCanisterId>,
    drives_directory: HashMap<u64, DriveCanisterId>,
}

impl State {
    fn new() -> Self {
        Self {
            drives_counter: 0,
            user_drive_directory: HashMap::new(),
            drives_directory: HashMap::new(),
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new());
}

#[update]
async fn create_drive(username: String) -> Result<String, String> {
    let caller: Principal = caller();
    if caller == Principal::anonymous() {
        return Err("Free users can only use local drives".to_string());
    }


    let sanitized_username = sanitize_username(&username);
    if !is_valid_username(&sanitized_username) {
        return Err("Invalid username format".to_string());
    }

    // Check if the user already has a drive
    if STATE.with(|state| state.borrow().user_drive_directory.contains_key(&caller)) {
        return Err("User already has a drive".to_string());
    }

    ic_cdk::println!("Creating drive for owner: {} with username: {}", caller, sanitized_username);

    let create_canister_arg = CreateCanisterArgument {
        settings: Some(ic_cdk::api::management_canister::main::CanisterSettings {
            controllers: Some(vec![ic_cdk::id(), caller]),
            compute_allocation: None,
            memory_allocation: None,
            freezing_threshold: None,
            reserved_cycles_limit: None,
        }),
    };

    let cycles_to_use = 1_000_000_000_000u128; // Adjust as needed

    match create_canister(create_canister_arg, cycles_to_use).await {
        Ok((canister_id_record,)) => {
            let drive_canister_id: DriveCanisterId = canister_id_record.canister_id;

            let arg = Encode!(&caller, &sanitized_username).unwrap();
            ic_cdk::println!("Encoded arguments: {:?}", arg);

            let install_code_arg = InstallCodeArgument {
                mode: CanisterInstallMode::Install,
                canister_id: drive_canister_id,
                wasm_module: DRIVE_WASM.to_vec(),
                arg,
            };

            ic_cdk::println!("Installing code with mode: {:?}", install_code_arg.mode);

            match install_code(install_code_arg).await {
                Ok(()) => {
                    ic_cdk::println!("Code installed successfully");
                    STATE.with(|state| {
                        let mut state = state.borrow_mut();
                        state.drives_counter += 1;
                        let drive_index = state.drives_counter;
                        state.drives_directory.insert(drive_index, drive_canister_id);
                        state.user_drive_directory.insert(caller, drive_canister_id);
                    });
                    Ok(drive_canister_id.to_string())
                }
                Err(e) => {
                    ic_cdk::println!("Failed to install code: {:?}", e);
                    Err(format!("Failed to install code: {:?}", e))
                }
            }
        }
        Err(e) => {
            ic_cdk::println!("Failed to create canister: {:?}", e);
            Err(format!("Failed to create canister: {:?}", e))
        }
    }
}


fn sanitize_username(username: &str) -> String {
    let re = Regex::new(r#"[/\\@:;'"`]"#).unwrap();
    let sanitized = re.replace_all(username, " ");
    sanitized.chars().take(32).collect::<String>().trim().to_string()
}

fn is_valid_username(username: &str) -> bool {
    let re = Regex::new(r"^[\p{L}\p{N}]+$").unwrap();
    re.is_match(username)
}

#[query]
fn get_user_drive() -> Option<String> {
    let caller: Principal = caller();
    STATE.with(|state| {
        state
            .borrow()
            .user_drive_directory
            .get(&caller)
            .map(|p| p.to_string())
    })
}

#[query]
fn get_total_drives() -> u64 {
    ic_cdk::println!("Getting total drives");
    let total_drives = STATE.with(|state| state.borrow().drives_counter);
    ic_cdk::println!("Total drives: {}", total_drives);
    total_drives
}

#[query]
fn get_drive_by_index(index: u64) -> Option<String> {
    STATE.with(|state| {
        state
            .borrow()
            .drives_directory
            .get(&index)
            .map(|p| p.to_string())
    })
}

#[query]
fn get_canister_balance() -> u64 {
    let balance = ic_cdk::api::canister_balance();
    ic_cdk::println!("Canister balance: {}", balance);
    balance
}
