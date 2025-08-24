
use pico_sdk::{client::{DefaultProverClient},init_logger};
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use tree_hash::{Hash256, TreeHash};

use ream_consensus::{
    attestation::Attestation,
    attester_slashing::AttesterSlashing,
    bls_to_execution_change::SignedBLSToExecutionChange,
    deposit::Deposit,
    electra::{
        beacon_block::BeaconBlock, beacon_state::BeaconState, execution_payload::ExecutionPayload,
    },
    proposer_slashing::ProposerSlashing,
    sync_aggregate::SyncAggregate,
    voluntary_exit::SignedVoluntaryExit,
};
use ream_lib::{file::ssz_from_file, input::{OperationInput, EpochProcessingType}, ssz::from_ssz_bytes,load_elf};

mod cli;
use cli::{fork::Fork, operation::OperationName};



/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Argument for STFs
    #[clap(flatten)]
    fork: cli::fork::ForkArgs,

    #[clap(flatten)]
    operation: cli::operation::OperationArgs,

    /// Verify the correctness of the state root by comparing against consensus-spec-tests' post_state
    #[clap(long, default_value_t = false)]
    compare_specs: bool,

    /// Verify the correctness of the state root by recomputing on the host
    #[clap(long, default_value_t = false)]
    compare_recompute: bool,

    #[clap(long)]
    excluded_cases: Vec<String>,
}

fn main() {
    setup_log();
    let elf = load_elf("../app/elf/riscv32im-pico-zkvm-elf");
    println!("Loaded elf, size: {} bytes", elf.len());
    let (fork, operation_name, excluded_cases, compare_specs, compare_recompute) = parse_args();
    let (base_dir, test_cases) = load_test_cases(&fork, &operation_name);

    for test_case in test_cases {
        if excluded_cases.contains(&test_case) {
            info!("Skipping test case: {test_case}");
            continue;
        }

        info!("[{operation_name}] Test case: {test_case}");

        let case_dir = &base_dir.join(&test_case);
        let input = prepare_input(&case_dir, &operation_name);
        let pre_state_ssz_bytes: Vec<u8> = ssz_from_file(&case_dir.join("pre.ssz_snappy"));

        // Setup the executor environment and inject inputs
        let client = DefaultProverClient::new(&elf);
        let mut stdin_builder = client.new_stdin_builder();
        stdin_builder
            .write(&pre_state_ssz_bytes.len());
        stdin_builder.write_slice(&pre_state_ssz_bytes);
        stdin_builder.write(&input);
            // .unwrap() 
            // .write_slice(&pre_state_ssz_bytes)
            // Operation input
            // .unwrap()
            // Build the environment
            // .build()
            // .unwrap();

        //
        // Prover setup & proving
        //
        let (cycles,raw_output) = client.emulate(stdin_builder);
        let (prefix_bytes, root_bytes) = raw_output.split_at(8);
        let state_root = Hash256::from_slice(root_bytes);

        println!("Execution complete in {} cycles",cycles);
        println!("Output size: {} bytes", state_root.len());
        println!("Output: {:#?}", state_root);
        assert_state_root_matches_specs(&state_root, &pre_state_ssz_bytes, &case_dir);
        // let publicValues=deserialize::<Vec<u8>>(&output);
        //
        // Proof verification
        //

        // let receipt = prove_info.receipt;
        // let new_state_root = receipt.journal.decode::<Hash256>().unwrap();

        // info!("Seal size: {:#?}", receipt.seal_size());
        // info!("Receipt: {:#?}", receipt);
        // info!("New state root: {:?}", new_state_root);

        // receipt.verify(CONSENSUS_STF_ID).unwrap();
        // info!("Verfication successful. Proof is valid.");

        //
        // Compare proofs against references (consensus-spec-tests or recompute on host)
        //

        // if compare_specs {
        //     info!("Comparing the root against consensus-spec-tests post_state");
        //     assert_state_root_matches_specs(&new_state_root, &pre_state_ssz_bytes, &case_dir);
        // }

        // if compare_recompute {
        //     info!("Comparing the root by recomputing on host");
        //     assert_state_root_matches_recompute(&new_state_root, &pre_state_ssz_bytes, &input);
        // }

        info!("----- Cycle Tracker End -----");
    }
}

fn setup_log() {
    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    }

    // Initialize tracing. In order to view logs, run `RUST_LOG=info cargo run`
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();
}

fn parse_args() -> (Fork, OperationName, Vec<String>, bool, bool) {
    let args = Args::parse();

    (
        args.fork.fork,
        args.operation.operation_name,
        args.excluded_cases,
        args.compare_specs,
        args.compare_recompute,
    )
}

fn prepare_input(case_dir: &PathBuf, operation_name: &OperationName) -> OperationInput {
    if operation_name.is_epoch_processing() {
        // For epoch processing, we don't need input files, just the processing type
        OperationInput::EpochProcessing(operation_name.to_epoch_processing_type().unwrap())
    } else if operation_name.is_process_slot() {
        // For process_slot, we don't need input files
        OperationInput::ProcessSlot
    } else {
        let input_path = &case_dir.join(format!("{}.ssz_snappy", operation_name.to_input_name()));

        match operation_name {
            OperationName::Attestation => OperationInput::Attestation(ssz_from_file(input_path)),
            OperationName::AttesterSlashing => {
                OperationInput::AttesterSlashing(ssz_from_file(input_path))
            }
            OperationName::BlockHeader => OperationInput::BeaconBlock(ssz_from_file(input_path)),
            OperationName::BLSToExecutionChange => {
                OperationInput::SignedBLSToExecutionChange(ssz_from_file(input_path))
            }
            OperationName::Deposit => OperationInput::Deposit(ssz_from_file(input_path)),
            OperationName::ExecutionPayload => {
                OperationInput::BeaconBlockBody(ssz_from_file(input_path))
            }
            OperationName::ProposerSlashing => {
                OperationInput::ProposerSlashing(ssz_from_file(input_path))
            }
            OperationName::SyncAggregate => OperationInput::SyncAggregate(ssz_from_file(input_path)),
            OperationName::VoluntaryExit => {
                OperationInput::SignedVoluntaryExit(ssz_from_file(input_path))
            }
            OperationName::Withdrawals => OperationInput::ExecutionPayload(ssz_from_file(input_path)),
            // Epoch processing and process_slot operations are handled above
            _ => unreachable!("Epoch processing and process_slot operations should be handled above"),
        }
    }
}

fn load_test_cases(fork: &Fork, operation_name: &OperationName) -> (PathBuf, Vec<String>) {
    // These assets are from consensus-specs repo.
    let test_case_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("mainnet")
        .join("tests")
        .join("mainnet");

    if !std::path::Path::new(&test_case_dir).exists() {
        error!("Error: You must first download test data via `make download`");
        std::process::exit(1);
    }

    let base_dir = if operation_name.is_epoch_processing() {
        // Epoch processing tests are in epoch_processing directory
        test_case_dir
            .join(format!("{}", fork))
            .join("epoch_processing")
            .join(format!("{}", operation_name))
            .join("pyspec_tests")
    } else if operation_name.is_process_slot() {
        // Process slot tests are in sanity/slots directory
        test_case_dir
            .join(format!("{}", fork))
            .join("sanity")
            .join("slots")
            .join("pyspec_tests")
    } else {
        // Regular operations are in operations directory
        test_case_dir
            .join(format!("{}", fork))
            .join("operations")
            .join(format!("{}", operation_name))
            .join("pyspec_tests")
    };

    let test_cases = ream_lib::file::get_test_cases(&base_dir);

    (base_dir, test_cases)
}

fn assert_state_root_matches_specs(
    new_state_root: &Hash256,
    pre_state_ssz_bytes: &[u8],
    case_dir: &PathBuf,
) {
    let post_state_opt: Option<BeaconState> = {
        if case_dir.join("post.ssz_snappy").exists() {
            let ssz_bytes: Vec<u8> = ssz_from_file(&case_dir.join("post.ssz_snappy"));
            Some(from_ssz_bytes(&ssz_bytes).unwrap())
        } else {
            None
        }
    };

    match post_state_opt {
        // If the specs provide post_state, compare the computed root against post_state's root
        Some(post_state) => {
            info!("post_state provided. The state root should be mutated.");
            assert_eq!(*new_state_root, post_state.tree_hash_root());
            info!("Execution is correct! State mutated and the roots match.");
        }
        // If the specs does not contain a post_state, compare the computed root against pre_state's root
        None => {
            info!("post_state not provided. The state root should not be mutated.");
            let pre_state: BeaconState = from_ssz_bytes(&pre_state_ssz_bytes).unwrap();
            assert_eq!(*new_state_root, pre_state.tree_hash_root());
            info!("Execution is correct! State should not be mutated and the roots match.");
        }
    }
}

fn assert_state_root_matches_recompute(
    new_state_root: &Hash256,
    pre_state_ssz_bytes: &[u8],
    input: &OperationInput,
) {
    let mut state: BeaconState = from_ssz_bytes(&pre_state_ssz_bytes).unwrap();

    match input {
        OperationInput::Attestation(ssz_bytes) => {
            let attestation: Attestation = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_attestation(&attestation);
        }
        OperationInput::AttesterSlashing(ssz_bytes) => {
            let attester_slashing: AttesterSlashing = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_attester_slashing(&attester_slashing);
        }
        OperationInput::BeaconBlock(ssz_bytes) => {
            let block: BeaconBlock = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_block_header(&block);
        }
        OperationInput::SignedBLSToExecutionChange(ssz_bytes) => {
            let bls_change: SignedBLSToExecutionChange = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_bls_to_execution_change(&bls_change);
        }
        OperationInput::Deposit(ssz_bytes) => {
            let deposit: Deposit = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_deposit(&deposit);
        }
        OperationInput::BeaconBlockBody(_ssz_bytes) => {
            panic!("Not implemented");
            // let block_body: BeaconBlockBody = from_ssz_bytes(&ssz_bytes).unwrap();
            // let _ = state.process_execution_payload(&block_body);
        }
        OperationInput::ProposerSlashing(ssz_bytes) => {
            let proposer_slashing: ProposerSlashing = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_proposer_slashing(&proposer_slashing);
        }
        OperationInput::SyncAggregate(ssz_bytes) => {
            let sync_aggregate: SyncAggregate = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_sync_aggregate(&sync_aggregate);
        }
        OperationInput::SignedVoluntaryExit(ssz_bytes) => {
            let voluntary_exit: SignedVoluntaryExit = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_voluntary_exit(&voluntary_exit);
        }
        OperationInput::ExecutionPayload(ssz_bytes) => {
            let execution_payload: ExecutionPayload = from_ssz_bytes(&ssz_bytes).unwrap();
            let _ = state.process_withdrawals(&execution_payload);
        }
        OperationInput::EpochProcessing(epoch_type) => {
            match epoch_type {
                EpochProcessingType::JustificationAndFinalization => {
                    let _ = state.process_justification_and_finalization();
                }
                EpochProcessingType::InactivityUpdates => {
                    let _ = state.process_inactivity_updates();
                }
                EpochProcessingType::RewardsAndPenalties => {
                    let _ = state.process_rewards_and_penalties();
                }
                EpochProcessingType::RegistryUpdates => {
                    let _ = state.process_registry_updates();
                }
                EpochProcessingType::Slashings => {
                    let _ = state.process_slashings();
                }
                EpochProcessingType::Eth1DataReset => {
                    let _ = state.process_eth1_data_reset();
                }
                EpochProcessingType::PendingDeposits => {
                    let _ = state.process_pending_deposits();
                }
                EpochProcessingType::PendingConsolidations => {
                    let _ = state.process_pending_consolidations();
                }
                EpochProcessingType::EffectiveBalanceUpdates => {
                    let _ = state.process_effective_balance_updates();
                }
                EpochProcessingType::SlashingsReset => {
                    let _ = state.process_slashings_reset();
                }
                EpochProcessingType::RandaoMixesReset => {
                    let _ = state.process_randao_mixes_reset();
                }
                EpochProcessingType::HistoricalSummariesUpdate => {
                    let _ = state.process_historical_summaries_update();
                }
                EpochProcessingType::ParticipationFlagUpdates => {
                    let _ = state.process_participation_flag_updates();
                }
                EpochProcessingType::SyncCommitteeUpdates => {
                    let _ = state.process_sync_committee_updates();
                }
            }
        }
        OperationInput::ProcessSlot => {
            let _ = state.process_slot();
        }
    }

    let recomputed_state_root = state.tree_hash_root();

    println!("recomputed_state_root: {}", recomputed_state_root);
    println!("new_state_root: {}", new_state_root);

    assert_eq!(*new_state_root, recomputed_state_root);
    info!("Execution is correct! State roots match host's recomputed state root.");
}
