// logic_tests.rs

use candid::{Encode, Decode, CandidType, Principal};
use ic_agent::{Agent, identity::AnonymousIdentity, agent::http_transport::ReqwestHttpReplicaV2Transport};
use ic_agent::export::Principal as AgentPrincipal;
use std::str::FromStr;

use crate::{FolderMetadata, StorageLocationEnum, DriveFullFilePath, UserID, StateSnapshot, FileMetadata};

const LOCAL_CANISTER_ID: &str = "bkyz2-fmaaa-aaaaa-qaaaq-cai"; // Replace with your local canister ID

async fn setup() -> (Agent, AgentPrincipal) {
    let url = "http://127.0.0.1:4943".to_string();
    let transport = ReqwestHttpReplicaV2Transport::create(url).expect("Failed to create transport");
    
    let agent = Agent::builder()
        .with_transport(transport)
        .with_identity(AnonymousIdentity)
        .build()
        .expect("Failed to build agent");

    agent.fetch_root_key().await.expect("Failed to fetch root key");

    let canister_id = AgentPrincipal::from_str(LOCAL_CANISTER_ID).unwrap();
    
    (agent, canister_id)
}

async fn clear_all_data(agent: &Agent, canister_id: &AgentPrincipal) -> Result<(), String> {
    let snapshot_response = agent.query(canister_id, "snapshot_hashtables")
        .with_arg(&Encode!().unwrap())
        .call().await
        .map_err(|e| format!("Failed to call snapshot_hashtables: {:?}", e))?;

    let snapshot: StateSnapshot = Decode!(&snapshot_response, StateSnapshot)
        .map_err(|e| format!("Failed to decode snapshot: {:?}", e))?;

    // Delete files first
    for (file_id, _) in snapshot.file_uuid_to_metadata {
        let delete_args = Encode!(&file_id).map_err(|e| format!("Failed to encode delete arguments: {:?}", e))?;
        agent.update(canister_id, "delete_file")
            .with_arg(&delete_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to delete file: {:?}", e))?;
    }

    // Then delete folders (from leaf to root), but preserve the root folder
    let mut folders: Vec<_> = snapshot.folder_uuid_to_metadata.into_iter().collect();
    folders.sort_by_key(|(_, folder)| std::cmp::Reverse(folder.full_folder_path.len()));

    for (folder_id, folder_metadata) in folders {
        if folder_metadata.full_folder_path == "BrowserCache::" {
            // Skip deleting the root folder
            continue;
        }
        let delete_args = Encode!(&folder_id).map_err(|e| format!("Failed to encode delete arguments: {:?}", e))?;
        agent.update(canister_id, "delete_folder")
            .with_arg(&delete_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to delete folder '{}': {:?}", folder_metadata.full_folder_path, e))?;
    }

    // Ensure that the root folder exists
    let root_path = "BrowserCache::".to_string();
    let snapshot_after_delete = get_snapshot(agent, canister_id).await?;
    let root_exists = snapshot_after_delete.full_folder_path_to_uuid.contains_key(&root_path);
    if !root_exists {
        // Attempt to create the root folder by creating a dummy subfolder
        let dummy_folder_path = "BrowserCache::dummy_root".to_string();
        let storage_location = StorageLocationEnum::BrowserCache;

        let create_args = Encode!(&dummy_folder_path, &storage_location)
            .map_err(|e| format!("Failed to encode dummy folder arguments: {:?}", e))?;
        agent.update(canister_id, "create_folder")
            .with_arg(&create_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to create dummy folder to ensure root exists: {:?}", e))?;

        // Optionally, delete the dummy folder to clean up
        let get_dummy_folder_response = agent.query(canister_id, "get_folder_by_path")
            .with_arg(&Encode!(&dummy_folder_path).unwrap())
            .call().await
            .map_err(|e| format!("Failed to call get_folder_by_path for dummy folder: {:?}", e))?;

        let dummy_folder: Option<FolderMetadata> = Decode!(&get_dummy_folder_response, Option<FolderMetadata>)
            .map_err(|e| format!("Failed to decode dummy folder response: {:?}", e))?;

        if let Some(folder) = dummy_folder {
            let delete_args = Encode!(&folder.id).map_err(|e| format!("Failed to encode delete arguments: {:?}", e))?;
            agent.update(canister_id, "delete_folder")
                .with_arg(&delete_args)
                .call_and_wait()
                .await
                .map_err(|e| format!("Failed to delete dummy folder: {:?}", e))?;
        }
    }

    Ok(())
}


#[tokio::test]
async fn test_ping() -> Result<(), String> {
    let (agent, canister_id) = setup().await;

    let response = agent.query(&canister_id, "ping")
        .with_arg(&Encode!().unwrap())
        .call().await
        .map_err(|e| format!("Failed to call ping: {:?}", e))?;

    let result: String = Decode!(&response, String)
        .map_err(|e| format!("Failed to decode ping response: {:?}", e))?;
    
    assert_eq!(result, "pong");
    Ok(())
}

#[tokio::test]
async fn test_create_folder() -> Result<(), String> {
    let (agent, canister_id) = setup().await;
    clear_all_data(&agent, &canister_id).await?;

    // Log the initial state
    let snapshot = get_snapshot(&agent, &canister_id).await?;
    println!("Initial state: {:?}", snapshot);

    let full_folder_path = "BrowserCache::test_folder1".to_string();
    let storage_location = StorageLocationEnum::BrowserCache;

    // Create folder
    let create_args = Encode!(&full_folder_path, &storage_location)
        .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;

    let create_response = agent
        .update(&canister_id, "create_folder")
        .with_arg(&create_args)
        .call_and_wait()
        .await
        .map_err(|e| format!("Failed to call create_folder: {:?}", e))?;

    let result: Result<FolderMetadata, String> = Decode!(&create_response, Result<FolderMetadata, String>)
        .map_err(|e| format!("Failed to decode create_folder response: {:?}", e))?;

    // Log the final state
    let snapshot = get_snapshot(&agent, &canister_id).await?;
    println!("Final state: {:?}", snapshot);

    match result {
        Ok(folder_metadata) => {
            println!("Folder created successfully: {:?}", folder_metadata);
            assert_eq!(folder_metadata.full_folder_path, "BrowserCache::test_folder1/");
            Ok(())
        },
        Err(e) => Err(format!("Failed to create folder: {}", e)),
    }
}

// Helper function to get the current state
async fn get_snapshot(agent: &Agent, canister_id: &AgentPrincipal) -> Result<StateSnapshot, String> {
    let snapshot_response = agent.query(canister_id, "snapshot_hashtables")
        .with_arg(&Encode!().unwrap())
        .call().await
        .map_err(|e| format!("Failed to call snapshot_hashtables: {:?}", e))?;

    Decode!(&snapshot_response, StateSnapshot)
        .map_err(|e| format!("Failed to decode snapshot: {:?}", e))
}

#[tokio::test]
async fn test_create_folders_with_subfolders() -> Result<(), String> {
    let (agent, canister_id) = setup().await;
    clear_all_data(&agent, &canister_id).await?;

    let folders = vec![
        "BrowserCache::folder1",
        "BrowserCache::folder1/subfolder1",
        "BrowserCache::folder1/subfolder2",
        "BrowserCache::folder2",
        "BrowserCache::folder2/subfolder1/subsubfolder1",
    ];

    for folder_path in folders {
        let create_args = Encode!(&folder_path, &StorageLocationEnum::BrowserCache)
            .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;

        let create_response = agent
            .update(&canister_id, "create_folder")
            .with_arg(&create_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to call create_folder: {:?}", e))?;

        let result: Result<FolderMetadata, String> = Decode!(&create_response, Result<FolderMetadata, String>)
            .map_err(|e| format!("Failed to decode create_folder response: {:?}", e))?;

        match result {
            Ok(folder_metadata) => {
                println!("Folder created successfully: {:?}", folder_metadata);
                assert_eq!(folder_metadata.full_folder_path, format!("{}/", folder_path));
            },
            Err(e) => return Err(format!("Failed to create folder: {}", e)),
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_upload_files() -> Result<(), String> {
    let (agent, canister_id) = setup().await;
    clear_all_data(&agent, &canister_id).await?;

    // Create a folder structure
    let create_folder_args = Encode!(&"BrowserCache::test_folder/subfolder", &StorageLocationEnum::BrowserCache)
        .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;
    agent.update(&canister_id, "create_folder")
        .with_arg(&create_folder_args)
        .call_and_wait()
        .await
        .map_err(|e| format!("Failed to call create_folder: {:?}", e))?;

    // Upload files
    let files = vec![
        "BrowserCache::test_folder/file1.txt",
        "BrowserCache::test_folder/subfolder/file2.txt",
    ];

    for file_path in files {
        let upload_args = Encode!(&file_path, &StorageLocationEnum::BrowserCache)
            .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;

        let upload_response = agent
            .update(&canister_id, "upsert_file_to_hash_tables")
            .with_arg(&upload_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to call upsert_file_to_hash_tables: {:?}", e))?;

        let file_id: String = Decode!(&upload_response, String)
            .map_err(|e| format!("Failed to decode upsert_file_to_hash_tables response: {:?}", e))?;

        println!("File uploaded successfully: {}", file_id);

        // Verify file exists
        let get_file_args = Encode!(&file_path).map_err(|e| format!("Failed to encode arguments: {:?}", e))?;
        let get_file_response = agent
            .query(&canister_id, "get_file_by_path")
            .with_arg(&get_file_args)
            .call()
            .await
            .map_err(|e| format!("Failed to call get_file_by_path: {:?}", e))?;

        let file_metadata: Option<FileMetadata> = Decode!(&get_file_response, Option<FileMetadata>)
            .map_err(|e| format!("Failed to decode get_file_by_path response: {:?}", e))?;

        assert!(file_metadata.is_some(), "File not found: {}", file_path);
    }

    Ok(())
}

#[tokio::test]
async fn test_recursive_delete() -> Result<(), String> {
    let (agent, canister_id) = setup().await;
    clear_all_data(&agent, &canister_id).await?;

    // Verify root folder exists
    let snapshot = get_snapshot(&agent, &canister_id).await?;
    let root_folders: Vec<&FolderMetadata> = snapshot.folder_uuid_to_metadata.values()
        .filter(|folder| folder.parent_folder_uuid.is_none())
        .collect();
    println!("Root folders after clear_all_data: {:?}", root_folders);

    // Proceed with creating folders and files
    let folders = vec![
        "BrowserCache::test_folder2/",
        "BrowserCache::test_folder2/subfolder1/",
        "BrowserCache::test_folder2/subfolder1/subsubfolder1/",
        "BrowserCache::test_folder2/subfolder2/",
    ];

    // Lists to keep track of created folder and file paths
    let mut created_folders = Vec::new();
    let mut created_files = Vec::new();

    for folder_path in &folders {
        let create_args = Encode!(&folder_path, &StorageLocationEnum::BrowserCache)
            .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;

        let create_response = agent.update(&canister_id, "create_folder")
            .with_arg(&create_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to call create_folder: {:?}", e))?;

        let result: Result<FolderMetadata, String> = Decode!(&create_response, Result<FolderMetadata, String>)
            .map_err(|e| format!("Failed to decode create_folder response: {:?}", e))?;

        match result {
            Ok(folder_metadata) => {
                println!("Folder created successfully: {:?}", folder_metadata);
                assert_eq!(folder_metadata.full_folder_path, format!("{}", folder_path));
                created_folders.push(folder_metadata.full_folder_path.clone());
            },
            Err(e) => return Err(format!("Failed to create folder: {}", e)),
        }
    }

    let files = vec![
        "BrowserCache::test_folder2/file1.txt",
        "BrowserCache::test_folder2/subfolder1/file2.txt",
        "BrowserCache::test_folder2/subfolder1/subsubfolder1/file3.txt",
        "BrowserCache::test_folder2/subfolder2/file4.txt",
    ];

    for file_path in &files {
        let upload_args = Encode!(&file_path, &StorageLocationEnum::BrowserCache)
            .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;

        let upload_response = agent.update(&canister_id, "upsert_file_to_hash_tables")
            .with_arg(&upload_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to call upsert_file_to_hash_tables: {:?}", e))?;

        let file_id: String = Decode!(&upload_response, String)
            .map_err(|e| format!("Failed to decode upsert_file_to_hash_tables response: {:?}", e))?;

        println!("File uploaded successfully: {}", file_id);
        created_files.push(file_path.to_string());
    }

    // Get the root folder UUID
    let get_folder_args = Encode!(&"BrowserCache::test_folder2/").map_err(|e| format!("Failed to encode arguments: {:?}", e))?;
    let get_folder_response = agent.query(&canister_id, "get_folder_by_path")
        .with_arg(&get_folder_args)
        .call()
        .await
        .map_err(|e| format!("Failed to call get_folder_by_path: {:?}", e))?;

    let root_folder: Option<FolderMetadata> = Decode!(&get_folder_response, Option<FolderMetadata>)
        .map_err(|e| format!("Failed to decode get_folder_by_path response: {:?}", e))?;

    println!("--- Root folder: {:?}", root_folder);

    let root_folder_id = root_folder.ok_or("Root folder not found")?.id;

    // Delete the root folder recursively
    let delete_args = Encode!(&root_folder_id).map_err(|e| format!("Failed to encode delete arguments: {:?}", e))?;
    agent.update(&canister_id, "delete_folder")
        .with_arg(&delete_args)
        .call_and_wait()
        .await
        .map_err(|e| format!("Failed to delete folder: {:?}", e))?;

    // Verify that the specific folders and files are deleted
    let snapshot = get_snapshot(&agent, &canister_id).await?;
    println!("----- Snapshot after deletion: {:?}", snapshot);

    for folder_path in &created_folders {
        assert!(
            !snapshot.full_folder_path_to_uuid.contains_key(folder_path),
            "Folder path '{}' was not deleted", folder_path
        );
    }

    for file_path in &created_files {
        assert!(
            !snapshot.full_file_path_to_uuid.contains_key(file_path),
            "File path '{}' was not deleted", file_path
        );
    }

    // Optionally, verify that other data still exists
    // Example:
    // assert!(snapshot.folder_uuid_to_metadata.len() > created_folders.len(), "Other folders were unexpectedly deleted");
    // assert!(snapshot.file_uuid_to_metadata.len() > created_files.len(), "Other files were unexpectedly deleted");

    Ok(())
}

#[tokio::test]
async fn test_rename_folder_with_subfolders_and_files() -> Result<(), String> {
    let (agent, canister_id) = setup().await;
    clear_all_data(&agent, &canister_id).await?;

    // Verify root folder exists
    let snapshot = get_snapshot(&agent, &canister_id).await?;
    let root_folders: Vec<&FolderMetadata> = snapshot.folder_uuid_to_metadata.values()
        .filter(|folder| folder.parent_folder_uuid.is_none())
        .collect();
    println!("Root folders after clear_all_data: {:?}", root_folders);

    // Create folders and files
    let folders = vec![
        "BrowserCache::old_folder/",
        "BrowserCache::old_folder/subfolder1/",
        "BrowserCache::old_folder/subfolder2/sub3/",
    ];

    for folder_path in folders {
        let create_args = Encode!(&folder_path, &StorageLocationEnum::BrowserCache)
            .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;

        let create_response = agent.update(&canister_id, "create_folder")
            .with_arg(&create_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to call create_folder: {:?}", e))?;

        let result: Result<FolderMetadata, String> = Decode!(&create_response, Result<FolderMetadata, String>)
            .map_err(|e| format!("Failed to decode create_folder response: {:?}", e))?;

        match result {
            Ok(folder_metadata) => {
                println!("Folder created successfully: {:?}", folder_metadata);
                assert_eq!(folder_metadata.full_folder_path, format!("{}", folder_path));
            },
            Err(e) => return Err(format!("Failed to create folder: {}", e)),
        }
    }

    let files = vec![
        "BrowserCache::old_folder/file1.txt",
        "BrowserCache::old_folder/subfolder1/file2.txt",
        "BrowserCache::old_folder/subfolder2/sub3/file3.txt",
    ];

    for file_path in files {
        let upload_args = Encode!(&file_path, &StorageLocationEnum::BrowserCache)
            .map_err(|e| format!("Failed to encode arguments: {:?}", e))?;

        let upload_response = agent.update(&canister_id, "upsert_file_to_hash_tables")
            .with_arg(&upload_args)
            .call_and_wait()
            .await
            .map_err(|e| format!("Failed to call upsert_file_to_hash_tables: {:?}", e))?;

        let file_id: String = Decode!(&upload_response, String)
            .map_err(|e| format!("Failed to decode upsert_file_to_hash_tables response: {:?}", e))?;

        println!("File uploaded successfully: {}", file_id);
    }

    // Get the root folder UUID
    let get_folder_args = Encode!(&"BrowserCache::old_folder/").map_err(|e| format!("Failed to encode arguments: {:?}", e))?;
    let get_folder_response = agent.query(&canister_id, "get_folder_by_path")
        .with_arg(&get_folder_args)
        .call()
        .await
        .map_err(|e| format!("Failed to call get_folder_by_path: {:?}", e))?;

    let root_folder: Option<FolderMetadata> = Decode!(&get_folder_response, Option<FolderMetadata>)
        .map_err(|e| format!("Failed to decode get_folder_by_path response: {:?}", e))?;

    let root_folder_id = root_folder.ok_or("Root folder not found")?.id;

    // Rename the root folder
    let rename_args = Encode!(&root_folder_id, &"new_folder".to_string()).map_err(|e| format!("Failed to encode rename arguments: {:?}", e))?;
    let rename_response = agent.update(&canister_id, "rename_folder")
        .with_arg(&rename_args)
        .call_and_wait()
        .await
        .map_err(|e| format!("Failed to rename folder: {:?}", e))?;

    let rename_result: Result<(), String> = Decode!(&rename_response, Result<(), String>)
        .map_err(|e| format!("Failed to decode rename_folder response: {:?}", e))?;

    match rename_result {
        Ok(_) => {
            println!("Folder renamed successfully.");

            // Verify that all paths are updated
            let snapshot = get_snapshot(&agent, &canister_id).await?;
            println!("Snapshot after renaming: {:?}", snapshot);

            // Check folder paths
            assert!(snapshot.full_folder_path_to_uuid.contains_key("BrowserCache::new_folder/"));
            assert!(snapshot.full_folder_path_to_uuid.contains_key("BrowserCache::new_folder/subfolder1/"));
            assert!(snapshot.full_folder_path_to_uuid.contains_key("BrowserCache::new_folder/subfolder2/"));
            assert!(!snapshot.full_folder_path_to_uuid.contains_key("BrowserCache::old_folder/"));

            // Check file paths
            assert!(snapshot.full_file_path_to_uuid.contains_key("BrowserCache::new_folder/file1.txt"));
            assert!(snapshot.full_file_path_to_uuid.contains_key("BrowserCache::new_folder/subfolder1/file2.txt"));
            assert!(snapshot.full_file_path_to_uuid.contains_key("BrowserCache::new_folder/subfolder2/sub3/file3.txt"));
            assert!(!snapshot.full_file_path_to_uuid.contains_key("BrowserCache::old_folder/file1.txt"));

            Ok(())
        },
        Err(e) => Err(format!("Failed to rename folder: {}", e)),
    }
}
