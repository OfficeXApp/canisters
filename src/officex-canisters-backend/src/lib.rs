// lib.rs

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use regex::Regex;
type FolderUUID = String;
type FileUUID = String;
type DriveFullFilePath = String;
type Tag = String;
type UserID = Principal;
use std::cell::Cell;
use sha2::{Sha256, Digest};



#[derive(CandidType, Serialize, Deserialize, Clone, Debug)]
struct StateSnapshot {
    folder_uuid_to_metadata: HashMap<FolderUUID, FolderMetadata>,
    file_uuid_to_metadata: HashMap<FileUUID, FileMetadata>,
    full_folder_path_to_uuid: HashMap<DriveFullFilePath, FolderUUID>,
    full_file_path_to_uuid: HashMap<DriveFullFilePath, FileUUID>,
    owner: Principal,
    username: String,
}

#[derive(Clone, PartialEq, Eq, Hash, CandidType, Serialize, Deserialize, Debug)]
enum StorageLocationEnum {
    BrowserCache,
    HardDrive,
    Web3Storj,
}

impl fmt::Display for StorageLocationEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageLocationEnum::BrowserCache => write!(f, "BrowserCache"),
            StorageLocationEnum::HardDrive => write!(f, "HardDrive"),
            StorageLocationEnum::Web3Storj => write!(f, "Web3Storj"),
        }
    }
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct FolderMetadata {
    id: FolderUUID,
    original_folder_name: String,
    parent_folder_uuid: Option<FolderUUID>,
    subfolder_uuids: Vec<FolderUUID>,
    file_uuids: Vec<FileUUID>,
    full_folder_path: DriveFullFilePath,
    tags: Vec<Tag>,
    owner: UserID,
    created_date: u64, // ISO 8601 format
    storage_location: StorageLocationEnum,
    last_changed_unix_ms: u64,
    deleted: bool
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct FileMetadata {
    id: FileUUID,
    original_file_name: String,
    folder_uuid: FolderUUID,
    file_version: u32,
    prior_version: Option<FileUUID>,
    next_version: Option<FileUUID>,
    extension: String,
    full_file_path: DriveFullFilePath,
    tags: Vec<Tag>,
    owner: UserID,
    created_date: u64, // ISO 8601 format
    storage_location: StorageLocationEnum,
    file_size: u64,
    raw_url: String,
    last_changed_unix_ms: u64, 
    deleted: bool
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct State {
    folder_uuid_to_metadata: HashMap<FolderUUID, FolderMetadata>,
    file_uuid_to_metadata: HashMap<FileUUID, FileMetadata>,
    full_folder_path_to_uuid: HashMap<DriveFullFilePath, FolderUUID>,
    full_file_path_to_uuid: HashMap<DriveFullFilePath, FileUUID>,
    owner: Principal,
    username: String,
}


impl State {
    fn new(owner: Principal, username: String) -> Self {
        let sanitized_username = sanitize_username(&username);
        let formatted_username = format!("{}@{}", sanitized_username, owner.to_string());
        Self {
            folder_uuid_to_metadata: HashMap::new(),
            file_uuid_to_metadata: HashMap::new(),
            full_folder_path_to_uuid: HashMap::new(),
            full_file_path_to_uuid: HashMap::new(),
            owner,
            username: formatted_username,
        }
    }

    fn ping() -> String {
        "pong".to_string()
    }

    fn update_username(&mut self, new_username: String) -> Result<(), String> {
        let caller = ic_cdk::caller();
        if caller != self.owner {
            return Err("Only the owner can update the username".to_string());
        }
        let sanitized_username = sanitize_username(&new_username);
        if !is_valid_username(&sanitized_username) {
            return Err("Invalid username format".to_string());
        }
        let formatted_username = format!("{}@{}", sanitized_username, self.owner.to_string());
        self.username = formatted_username;
        Ok(())
    }

    pub fn create_folder(
        &mut self,
        full_folder_path: DriveFullFilePath,
        storage_location: StorageLocationEnum,
        user_id: UserID
    ) -> Result<FolderMetadata, String> {
        // Ensure the path ends with a slash
        let mut sanitized_path = Self::sanitize_file_path(&full_folder_path);
        if !sanitized_path.ends_with('/') {
            sanitized_path.push('/');
        }
    
        if sanitized_path.is_empty() {
            return Err(String::from("Invalid folder path"));
        }
    
        // Split the path into storage and folder parts
        let parts: Vec<&str> = sanitized_path.split("::").collect();
        if parts.len() < 2 {
            return Err(String::from("Invalid folder path format"));
        }
    
        let storage_part = parts[0];
        let folder_path = parts[1..].join("::");
    
        // Ensure the storage location matches
        if storage_part != storage_location.to_string() {
            return Err(String::from("Storage location mismatch"));
        }
    
        // Split the folder path into individual parts
        let path_parts: Vec<&str> = folder_path.split('/').filter(|&x| !x.is_empty()).collect();
    
    
        let mut current_path = format!("{}::", storage_part);
        let mut parent_folder_uuid = self.ensure_root_folder(&storage_location, &user_id);

        // root folder case
        if path_parts.is_empty() {
            return self.folder_uuid_to_metadata.get(&parent_folder_uuid).cloned().ok_or_else(|| "Parent folder not found".to_string());
        }
    
        // Iterate through path parts and create folders as needed
        for (i, part) in path_parts.iter().enumerate() {
            current_path.push_str(part);
            current_path.push('/');
    
            if !self.full_folder_path_to_uuid.contains_key(&current_path) {
                let new_folder_uuid = generate_unique_id();
                let new_folder = FolderMetadata {
                    id: new_folder_uuid.clone(),
                    original_folder_name: part.to_string(),
                    parent_folder_uuid: Some(parent_folder_uuid.clone()),
                    subfolder_uuids: Vec::new(),
                    file_uuids: Vec::new(),
                    full_folder_path: current_path.clone(),
                    tags: Vec::new(),
                    owner: user_id.clone(),
                    created_date: ic_cdk::api::time(),
                    storage_location: storage_location.clone(),
                    last_changed_unix_ms: ic_cdk::api::time() / 1_000_000,
                    deleted: false,
                };
    
                self.full_folder_path_to_uuid.insert(current_path.clone(), new_folder_uuid.clone());
                self.folder_uuid_to_metadata.insert(new_folder_uuid.clone(), new_folder.clone());
    
                // Update parent folder
                if let Some(parent_folder) = self.folder_uuid_to_metadata.get_mut(&parent_folder_uuid) {
                    parent_folder.subfolder_uuids.push(new_folder_uuid.clone());
                }
    
                parent_folder_uuid = new_folder_uuid;
    
                // If this is the last part, return the created folder
                if i == path_parts.len() - 1 {
                    return Ok(new_folder);
                }
            } else {
                parent_folder_uuid = self.full_folder_path_to_uuid[&current_path].clone();
            }
        }
    
        // If we've reached here, it means the folder already existed
        Err(String::from("Folder already exists"))
    }

    fn update_folder_file_uuids(&mut self, folder_uuid: &FolderUUID, file_uuid: &FileUUID, is_add: bool) {
        if let Some(folder) = self.folder_uuid_to_metadata.get_mut(folder_uuid) {
            if is_add {
                if !folder.file_uuids.contains(file_uuid) {
                    folder.file_uuids.push(file_uuid.clone());
                }
            } else {
                folder.file_uuids.retain(|uuid| uuid != file_uuid);
            }
        }
    }

    pub fn upsert_file_to_hash_tables(
        &mut self,
        file_path: String,
        storage_location: StorageLocationEnum,
        user_id: UserID,
    ) -> FileUUID {
        let sanitized_file_path = Self::sanitize_file_path(&file_path);
        let full_file_path = sanitized_file_path;
        let new_file_uuid = generate_unique_id();

        let (folder_path, file_name) = self.split_path(&full_file_path);
        let folder_uuid = self.ensure_folder_structure(&folder_path, storage_location.clone(), user_id);

        let existing_file_uuid = self.full_file_path_to_uuid.get(&full_file_path).cloned();

        let file_version = if let Some(existing_uuid) = &existing_file_uuid {
            let existing_file = self.file_uuid_to_metadata.get(existing_uuid).unwrap();
            existing_file.file_version + 1
        } else {
            1
        };

        let extension = file_name.rsplit('.').next().unwrap_or("").to_string();

        let file_metadata = FileMetadata {
            id: new_file_uuid.clone(),
            original_file_name: file_name,
            folder_uuid: folder_uuid.clone(),
            file_version,
            prior_version: existing_file_uuid.clone(),
            next_version: None,
            extension,
            full_file_path: full_file_path.clone(),
            tags: Vec::new(),
            owner: user_id,
            created_date: ic_cdk::api::time(),
            storage_location,
            file_size: 0,
            raw_url: String::new(),
            last_changed_unix_ms: ic_cdk::api::time() / 1_000_000,
            deleted: false,
        };

        // Update hashtables
        self.file_uuid_to_metadata.insert(new_file_uuid.clone(), file_metadata);
        self.full_file_path_to_uuid.insert(full_file_path, new_file_uuid.clone());

        // Update parent folder's file_uuids
        self.update_folder_file_uuids(&folder_uuid, &new_file_uuid, true);

        // Update prior version if it exists
        if let Some(existing_uuid) = existing_file_uuid {
            if let Some(existing_file) = self.file_uuid_to_metadata.get_mut(&existing_uuid) {
                existing_file.next_version = Some(new_file_uuid.clone());
            }
            // Remove the old file UUID from the parent folder
            self.update_folder_file_uuids(&folder_uuid, &existing_uuid, false);
        }

        new_file_uuid
    }

    fn get_folder_by_id(&self, folder_id: &FolderUUID) -> Option<&FolderMetadata> {
        self.folder_uuid_to_metadata.get(folder_id)
    }

    fn get_file_by_id(&self, file_id: &FileUUID) -> Option<&FileMetadata> {
        self.file_uuid_to_metadata.get(file_id)
    }

    fn get_folder_by_path(&self, path: &DriveFullFilePath) -> Option<&FolderMetadata> {
        self.full_folder_path_to_uuid
            .get(path)
            .and_then(|uuid| self.folder_uuid_to_metadata.get(uuid))
    }

    fn get_file_by_path(&self, path: &DriveFullFilePath) -> Option<&FileMetadata> {
        self.full_file_path_to_uuid
            .get(path)
            .and_then(|uuid| self.file_uuid_to_metadata.get(uuid))
    }

    fn rename_folder(&mut self, folder_id: FolderUUID, new_name: String) -> Result<(), String> {
        // Attempt to retrieve the folder metadata
        if let Some(folder) = self.folder_uuid_to_metadata.get_mut(&folder_id) {
            let old_path = folder.full_folder_path.clone();
            ic_cdk::println!("Old folder path: {}", old_path);
    
            // Split the path into storage and folder parts
            let parts: Vec<&str> = old_path.splitn(2, "::").collect();
            if parts.len() != 2 {
                return Err("Invalid folder structure".to_string());
            }
    
            let storage_part = parts[0].to_string();
            let folder_path = parts[1].trim_end_matches('/').to_string(); // Remove trailing slash
    
            // Perform path manipulation
            let path_parts: Vec<&str> = folder_path.rsplitn(2, '/').collect();
            let (parent_path, _current_folder_name) = match path_parts.len() {
                2 => (path_parts[1].to_string(), path_parts[0].to_string()),
                1 => (String::new(), path_parts[0].to_string()),
                _ => return Err("Invalid folder structure".to_string()),
            };
    
            // Construct the new folder path
            let new_folder_path = if parent_path.is_empty() {
                format!("{}::{}{}", storage_part, new_name, "/")
            } else {
                format!("{}::{}/{}{}", storage_part, parent_path, new_name, "/")
            };
    
            // Check if a folder with the new path already exists
            if self.full_folder_path_to_uuid.contains_key(&new_folder_path) {
                return Err("A folder with the new name already exists in the parent directory".to_string());
            }
    
            // Update folder metadata
            folder.original_folder_name = new_name.clone();
            folder.full_folder_path = new_folder_path.clone();
            folder.last_changed_unix_ms = ic_cdk::api::time() / 1_000_000;
    
            // Update path mappings
            ic_cdk::println!("Removing old path from full_folder_path_to_uuid: {}", old_path);
            self.full_folder_path_to_uuid.remove(&old_path);
    
            ic_cdk::println!("Inserting new path into full_folder_path_to_uuid: {}", new_folder_path);
            self.full_folder_path_to_uuid.insert(new_folder_path.clone(), folder_id.clone());
    
            // Update subfolder paths recursively
            self.update_subfolder_paths(&folder_id, &old_path, &new_folder_path);
    
            // If the folder has a parent, ensure the parent's subfolder_uuids include this folder
            if !parent_path.is_empty() {
                let parent_full_path = format!("{}::{}", storage_part, parent_path);
                if let Some(parent_uuid) = self.full_folder_path_to_uuid.get(&parent_full_path) {
                    if let Some(parent_folder) = self.folder_uuid_to_metadata.get_mut(parent_uuid) {
                        // Ensure the folder's UUID is present in the parent's subfolder_uuids
                        if !parent_folder.subfolder_uuids.contains(&folder_id) {
                            parent_folder.subfolder_uuids.push(folder_id.clone());
                            ic_cdk::println!("Added folder UUID to parent folder's subfolder_uuids");
                        }
                    }
                } else {
                    ic_cdk::println!("Parent folder not found for path: {}", parent_full_path);
                    return Err("Parent folder not found".to_string());
                }
            }
    
            ic_cdk::println!("Folder renamed successfully");
            Ok(())
        } else {
            Err("Folder not found".to_string())
        }
    }
    
    
    fn rename_file(&mut self, file_id: FileUUID, new_name: String) -> Result<(), String> {
        ic_cdk::println!(
            "Attempting to rename file. File ID: {}, New Name: {}",
            file_id,
            new_name
        );

        // Attempt to retrieve the file metadata
        if let Some(file) = self.file_uuid_to_metadata.get_mut(&file_id) {
            let old_path = file.full_file_path.clone();
            ic_cdk::println!("Old file path: {}", old_path);

            // Split the path into storage part and the rest
            let parts: Vec<&str> = old_path.splitn(2, "::").collect();
            if parts.len() != 2 {
                return Err("Invalid file structure".to_string());
            }

            let storage_part = parts[0].to_string();
            let file_path = parts[1].to_string();

            // Split the file path and replace the last part (file name)
            let path_parts: Vec<&str> = file_path.rsplitn(2, '/').collect();
            let new_path = if path_parts.len() > 1 {
                format!("{}::{}/{}", storage_part, path_parts[1], new_name)
            } else {
                format!("{}::{}", storage_part, new_name)
            };

            ic_cdk::println!("New file path: {}", new_path);

            // Check if a file with the new name already exists
            if self.full_file_path_to_uuid.contains_key(&new_path) {
                ic_cdk::println!("Error: A file with this name already exists");
                return Err("A file with this name already exists".to_string());
            }

            // Update file metadata
            file.original_file_name = new_name.clone();
            file.full_file_path = new_path.clone();
            file.last_changed_unix_ms = ic_cdk::api::time() / 1_000_000;
            file.extension = new_name
                .rsplit('.')
                .next()
                .unwrap_or("")
                .to_string();
            ic_cdk::println!("Updated file metadata: {:?}", file);

            // Update path mappings
            ic_cdk::println!(
                "Removing old path from full_file_path_to_uuid: {}",
                old_path
            );
            self.full_file_path_to_uuid.remove(&old_path);

            ic_cdk::println!(
                "Inserting new path into full_file_path_to_uuid: {}",
                new_path
            );
            self.full_file_path_to_uuid.insert(new_path, file_id.clone());

            ic_cdk::println!("File renamed successfully");
            Ok(())
        } else {
            ic_cdk::println!("Error: File not found. File ID: {}", file_id);
            Err("File not found".to_string())
        }
    }
    fn delete_folder(&mut self, folder_id: &FolderUUID) -> Result<(), String> {
        ic_cdk::println!("Attempting to delete folder. Folder ID: {}", folder_id);
        
        let (folder_path, subfolder_ids, file_ids) = if let Some(folder) = self.folder_uuid_to_metadata.get(folder_id) {
            (
                folder.full_folder_path.clone(),
                folder.subfolder_uuids.clone(),
                folder.file_uuids.clone()
            )
        } else {
            ic_cdk::println!("Error: Folder not found. Folder ID: {}", folder_id);
            return Err("Folder not found".to_string());
        };
        
            ic_cdk::println!("Folder found. Full path: {}", folder_path);
            
            ic_cdk::println!("Removing folder path from full_folder_path_to_uuid");
            self.full_folder_path_to_uuid.remove(&folder_path);

            // Recursively delete subfolders
            ic_cdk::println!("Deleting subfolders");
            for subfolder_id in subfolder_ids {
                ic_cdk::println!("Deleting subfolder: {}", subfolder_id);
                self.delete_folder(&subfolder_id)?;
            }

            // Delete files in this folder
            ic_cdk::println!("Deleting files in the folder");
            for file_id in file_ids {
                ic_cdk::println!("Deleting file: {}", file_id);
                self.delete_file(&file_id)?;
            }

            // Don't Remove folder from parent's subfolders list as we need the folder metadata.deleted to sync offline-cloud
            // if let Some(parent_id) = folder.parent_folder_uuid {
            //     ic_cdk::println!("Updating parent folder. Parent ID: {}", parent_id);
            //     if let Some(parent) = self.folder_uuid_to_metadata.get_mut(&parent_id) {
            //         parent.subfolder_uuids.retain(|id| id != folder_id);
            //         ic_cdk::println!("Updated parent's subfolder_uuids: {:?}", parent.subfolder_uuids);
            //     }
            // }

            // Mark the folder as deleted
            if let Some(folder) = self.folder_uuid_to_metadata.get_mut(folder_id) {
                folder.last_changed_unix_ms = ic_cdk::api::time() / 1_000_000;
                folder.deleted = true;
            }

            ic_cdk::println!("Folder deleted successfully");
            
            Ok(())
    }

    fn delete_file(&mut self, file_id: &FileUUID) -> Result<(), String> {
        ic_cdk::println!("Attempting to delete file. File ID: {}", file_id);
        
        let file = self.file_uuid_to_metadata.remove(file_id)
            .ok_or_else(|| {
                ic_cdk::println!("Error: File not found. File ID: {}", file_id);
                "File not found".to_string()
            })?;

        ic_cdk::println!("File found. Full path: {}", file.full_file_path);
        
        ic_cdk::println!("Removing file path from full_file_path_to_uuid --");
        self.full_file_path_to_uuid.remove(&file.full_file_path);

        // Don't Remove file from its parent folder's file list as we need the file metadata.deleted to sync offline-cloud
        // ic_cdk::println!("Updating parent folder. Folder UUID: {}", file.folder_uuid);
        // if let Some(parent) = self.folder_uuid_to_metadata.get_mut(&file.folder_uuid) {
        //     parent.file_uuids.retain(|id| id != file_id);
        //     ic_cdk::println!("Updated parent's file_uuids: {:?}", parent.file_uuids);
        // }

        // Handle versioning
        if let Some(prior_version) = &file.prior_version {
            ic_cdk::println!("Updating prior version. Prior version ID: {}", prior_version);
            if let Some(prior_file) = self.file_uuid_to_metadata.get_mut(prior_version) {
                prior_file.next_version = file.next_version.clone();
                ic_cdk::println!("Updated prior file's next_version: {:?}", prior_file.next_version);
            }
        }
        if let Some(next_version) = &file.next_version {
            ic_cdk::println!("Updating next version. Next version ID: {}", next_version);
            if let Some(next_file) = self.file_uuid_to_metadata.get_mut(next_version) {
                next_file.prior_version = file.prior_version.clone();
                ic_cdk::println!("Updated next file's prior_version: {:?}", next_file.prior_version);
            }
        }

        ic_cdk::println!("File deleted successfully");
        Ok(())
    }

    fn upsert_cloud_file_with_local_sync(&mut self, file_id: &FileUUID, file_metadata: &FileMetadata) -> Result<(FileUUID), String> {
        // overwrite the cloud file metadata with the latest version from offline client
        // must increment the file_version, and append the new file version with client submitted metadata (sanitized)
        let user_id = ic_cdk::caller();
        let existing_file = self.file_uuid_to_metadata.get(&file_id.clone()).unwrap().clone();

        let sanitized_new_file_path = Self::sanitize_file_path(&file_metadata.full_file_path);
        let new_full_file_path = sanitized_new_file_path;
        
        let new_file_uuid = generate_unique_id();
        
        let (new_folder_path, new_file_name) = self.split_path(&new_full_file_path);
        let folder_uuid = self.ensure_folder_structure(&new_folder_path, file_metadata.storage_location.clone(), user_id);

        let extension = new_file_name.rsplit('.').next().unwrap_or("").to_string();

         // Clean up version chain in folder
        if let Some(folder) = self.folder_uuid_to_metadata.get_mut(&folder_uuid) {
            let mut current_version = Some(file_id.clone());
            while let Some(version_id) = current_version {
                if let Some(version_file) = self.file_uuid_to_metadata.get(&version_id) {
                    folder.file_uuids.retain(|uuid| uuid != &version_id);
                    current_version = version_file.prior_version.clone();
                } else {
                    break;
                }
            }
        }

        let new_file_metadata = FileMetadata {
            id: new_file_uuid.clone(),
            original_file_name: new_file_name,
            folder_uuid: folder_uuid.clone(),
            file_version: existing_file.file_version + 1,
            prior_version: Some(existing_file.id.clone()),
            next_version: None,
            extension,
            full_file_path: new_full_file_path.clone(),
            tags: Vec::new(),
            owner: user_id,
            created_date: file_metadata.created_date,
            storage_location: file_metadata.storage_location.clone(),
            file_size: file_metadata.file_size,
            raw_url: file_metadata.raw_url.clone(),
            last_changed_unix_ms: file_metadata.last_changed_unix_ms | ic_cdk::api::time() / 1_000_000,
            deleted: file_metadata.deleted,
        };

        // Update hashtables
        self.file_uuid_to_metadata.insert(new_file_uuid.clone(), new_file_metadata);
        self.full_file_path_to_uuid.insert(new_full_file_path, new_file_uuid.clone());

        // // Update parent folder's file_uuids
        // self.update_folder_file_uuids(&folder_uuid, &new_file_uuid, true);
        // Update parent folder's file_uuids (only add new version)
        if let Some(folder) = self.folder_uuid_to_metadata.get_mut(&folder_uuid) {
            folder.file_uuids.retain(|uuid| uuid != &new_file_uuid);
            folder.file_uuids.push(new_file_uuid.clone());
        }

        // Update version chain
        if let Some(existing_file) = self.file_uuid_to_metadata.get_mut(&file_id.clone()) {
            existing_file.next_version = Some(new_file_uuid.clone());
        }

        return Ok((new_file_uuid.clone()));
    }
    fn upsert_cloud_folder_with_local_sync(&mut self, folder_id: &FolderUUID, folder_metadata: &FolderMetadata) -> Result<(FolderUUID), String> {
        // overwrite the cloud folder metadata with the latest version from offline client
        // no need to change folder versions, no version tracking on folders
        let existing_folder = self.folder_uuid_to_metadata.get_mut(&folder_id.clone()).unwrap();
        existing_folder.original_folder_name = folder_metadata.original_folder_name.clone();
        existing_folder.tags = folder_metadata.tags.clone();
        existing_folder.storage_location = folder_metadata.storage_location.clone();
        existing_folder.full_folder_path = folder_metadata.full_folder_path.clone();
        existing_folder.parent_folder_uuid = folder_metadata.parent_folder_uuid.clone();
        existing_folder.deleted = folder_metadata.deleted;
        existing_folder.last_changed_unix_ms = folder_metadata.last_changed_unix_ms | ic_cdk::api::time() / 1_000_000;
        return Ok((folder_id.clone()));
    }

    fn update_subfolder_paths(&mut self, folder_id: &FolderUUID, old_path: &str, new_path: &str) {
        if let Some(folder) = self.folder_uuid_to_metadata.get(folder_id).cloned() {
            for subfolder_id in &folder.subfolder_uuids {
                if let Some(subfolder) = self.folder_uuid_to_metadata.get_mut(subfolder_id) {
                    let old_subfolder_path = subfolder.full_folder_path.clone();
                    let new_subfolder_path = old_subfolder_path.replace(old_path, new_path);
                    
                    self.full_folder_path_to_uuid.remove(&old_subfolder_path);
                    subfolder.full_folder_path = new_subfolder_path.clone();
                    self.full_folder_path_to_uuid.insert(new_subfolder_path.clone(), subfolder_id.clone());
                    
                    self.update_subfolder_paths(subfolder_id, &old_subfolder_path, &new_subfolder_path);
                }
            }

            // Update file paths
            for file_id in &folder.file_uuids {
                if let Some(file) = self.file_uuid_to_metadata.get_mut(file_id) {
                    let old_file_path = file.full_file_path.clone();
                    let new_file_path = old_file_path.replace(old_path, new_path);
                    
                    self.full_file_path_to_uuid.remove(&old_file_path);
                    file.full_file_path = new_file_path.clone();
                    self.full_file_path_to_uuid.insert(new_file_path, file_id.clone());
                }
            }
        }
    }
    
    fn fetch_files_at_folder_path(&self, config: FetchFilesAtFolderPathConfig) -> FetchFilesResult {
        let FetchFilesAtFolderPathConfig { full_folder_path, limit, after } = config;
        
        if let Some(folder_uuid) = self.full_folder_path_to_uuid.get(&full_folder_path) {
            if let Some(folder) = self.folder_uuid_to_metadata.get(folder_uuid) {
                let mut folders = Vec::new();
                let mut files = Vec::new();

                // Collect subfolders
                for subfolder_uuid in &folder.subfolder_uuids {
                    if let Some(subfolder) = self.folder_uuid_to_metadata.get(subfolder_uuid) {
                        folders.push(subfolder.clone());
                    }
                }

                // Collect files
                for file_uuid in &folder.file_uuids {
                    if let Some(file) = self.file_uuid_to_metadata.get(file_uuid) {
                        files.push(file.clone());
                    }
                }

                // Apply pagination
                let total_items = folders.len() + files.len();
                let start = after as usize;
                let end = (start + limit as usize).min(total_items);

                let result_folders: Vec<FolderMetadata>;
                let result_files: Vec<FileMetadata>;

                if start < folders.len() {
                    let folders_end = end.min(folders.len());
                    result_folders = folders[start..folders_end].to_vec();
                    result_files = if end > folders.len() {
                        files[0..end - folders.len()].to_vec()
                    } else {
                        Vec::new()
                    };
                } else {
                    result_folders = Vec::new();
                    let files_start = start - folders.len();
                    result_files = files[files_start..end - folders.len()].to_vec();
                }

                let total_results = result_folders.len() + result_files.len();

                FetchFilesResult {
                    folders: result_folders,
                    files: result_files,
                    total: total_results as u32,
                    has_more: end < total_items,
                }
            } else {
                FetchFilesResult::empty()
            }
        } else {
            FetchFilesResult::empty()
        }
    }

    fn ensure_root_folder(&mut self, storage_location: &StorageLocationEnum, user_id: &UserID) -> FolderUUID {
        let root_path = format!("{}::", storage_location.to_string());
        if let Some(uuid) = self.full_folder_path_to_uuid.get(&root_path) {
            uuid.clone()
        } else {
            let root_folder_uuid = generate_unique_id();
            let root_folder = FolderMetadata {
                id: root_folder_uuid.clone(),
                original_folder_name: String::new(),
                parent_folder_uuid: None,
                subfolder_uuids: Vec::new(),
                file_uuids: Vec::new(),
                full_folder_path: root_path.clone(),
                tags: Vec::new(),
                owner: user_id.clone(),
                created_date: ic_cdk::api::time(),
                storage_location: storage_location.clone(),
                last_changed_unix_ms: ic_cdk::api::time() / 1_000_000,
                deleted: false,
            };

            self.full_folder_path_to_uuid.insert(root_path, root_folder_uuid.clone());
            self.folder_uuid_to_metadata.insert(root_folder_uuid.clone(), root_folder);

            root_folder_uuid
        }
    }

    pub fn ensure_folder_structure(
        &mut self,
        folder_path: &str,
        storage_location: StorageLocationEnum,
        user_id: UserID,
    ) -> FolderUUID {
        let path_parts: Vec<&str> = folder_path.split("::").collect();
        let mut current_path = format!("{}::", path_parts[0]);
        let mut parent_uuid = self.ensure_root_folder(&storage_location, &user_id);

        for part in path_parts[1].split('/').filter(|&p| !p.is_empty()) {
            current_path = format!("{}{}/", current_path, part);
            
            if !self.full_folder_path_to_uuid.contains_key(&current_path) {
                let new_folder_uuid = generate_unique_id();
                let new_folder = FolderMetadata {
                    id: new_folder_uuid.clone(),
                    original_folder_name: part.to_string(),
                    parent_folder_uuid: Some(parent_uuid.clone()),
                    subfolder_uuids: Vec::new(),
                    file_uuids: Vec::new(),
                    full_folder_path: current_path.clone(),
                    tags: Vec::new(),
                    owner: user_id,
                    created_date: ic_cdk::api::time(),
                    storage_location: storage_location.clone(),
                    last_changed_unix_ms: ic_cdk::api::time() / 1_000_000,
                    deleted: false,
                };

                self.full_folder_path_to_uuid.insert(current_path.clone(), new_folder_uuid.clone());
                self.folder_uuid_to_metadata.insert(new_folder_uuid.clone(), new_folder);

                // Update parent folder's subfolder_uuids
                if let Some(parent_folder) = self.folder_uuid_to_metadata.get_mut(&parent_uuid) {
                    if !parent_folder.subfolder_uuids.contains(&new_folder_uuid) {
                        parent_folder.subfolder_uuids.push(new_folder_uuid.clone());
                    }
                }

                parent_uuid = new_folder_uuid;
            } else {
                parent_uuid = self.full_folder_path_to_uuid[&current_path].clone();
            }
        }

        parent_uuid
    }

    fn sanitize_file_path(file_path: &str) -> String {
        let mut parts = file_path.splitn(2, "::");
        let storage_part = parts.next().unwrap_or("");
        let path_part = parts.next().unwrap_or("");
    
        let sanitized = path_part.replace(':', ";");

        // Compile a regex to match one or more consecutive slashes
        let re = Regex::new(r"/+").unwrap();
        let sanitized = re.replace_all(&sanitized, "/").to_string();

        // Remove leading and trailing slashes
        let sanitized = sanitized.trim_matches('/').to_string();

        // Additional sanitization can be performed here if necessary
    
        // Reconstruct the full path
        format!("{}::{}", storage_part, sanitized)
    }

    fn split_path(&self, full_path: &str) -> (String, String) {
        let parts: Vec<&str> = full_path.rsplitn(2, '/').collect();
        match parts.as_slice() {
            [file_name, folder_path] => (folder_path.to_string(), file_name.to_string()),
            [single_part] => {
                let storage_parts: Vec<&str> = single_part.splitn(2, "::").collect();
                match storage_parts.as_slice() {
                    [storage, file_name] => (format!("{}::", storage), file_name.to_string()),
                    _ => (String::new(), single_part.to_string()),
                }
            },
            _ => (String::new(), String::new()),
        }
    }


    fn snapshot_hashtables(&self) -> StateSnapshot {
        StateSnapshot {
            folder_uuid_to_metadata: self.folder_uuid_to_metadata.clone(),
            file_uuid_to_metadata: self.file_uuid_to_metadata.clone(),
            full_folder_path_to_uuid: self.full_folder_path_to_uuid.clone(),
            full_file_path_to_uuid: self.full_file_path_to_uuid.clone(),
            owner: self.owner,
            username: self.username.rsplit("@").next().unwrap_or("").to_string(),
        }
    }
}

fn generate_unique_id() -> String {
    let canister_id = ic_cdk::api::id().to_string();          // Canister's unique ID
    let current_time = ic_cdk::api::time();                   // Nanoseconds timestamp
    let caller = ic_cdk::api::caller().to_string();           // Principal of the caller
    
    // Increment the counter for every call
    ID_COUNTER.with(|counter| {
        let current_counter = counter.get();
        counter.set(current_counter + 1);

        // Create a unique string by combining deterministic inputs
        let input_string = format!("{}-{}-{}-{}", canister_id, current_time, caller, current_counter);

        // Use SHA256 to hash the input string and produce a compact, unique identifier
        let mut hasher = Sha256::new();
        hasher.update(input_string);
        format!("{:x}", hasher.finalize())
    })
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

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new(
        ic_cdk::api::caller(),
        "Anonymous".to_string()
    ));
    static ID_COUNTER: Cell<u64> = Cell::new(0);
}

#[ic_cdk::query]
fn ping() -> String {
    "pong".to_string()
}

#[ic_cdk::init]
fn init() {
    STATE.with(|state| {
        *state.borrow_mut() = State::new(
            ic_cdk::api::caller(),
            "Anonymous".to_string()
        );
    });
}

#[ic_cdk::update]
fn create_folder(full_folder_path: DriveFullFilePath, storage_location: StorageLocationEnum) -> Result<FolderMetadata, String> {
    let user_id = ic_cdk::caller();
    STATE.with(|state| state.borrow_mut().create_folder(full_folder_path, storage_location, user_id))
}

#[ic_cdk::update]
fn upsert_file_to_hash_tables(file_path: String, storage_location: StorageLocationEnum) -> FileUUID {
    let user_id = ic_cdk::caller();
    STATE.with(|state| state.borrow_mut().upsert_file_to_hash_tables(file_path, storage_location, user_id))
}


#[ic_cdk::query]
fn fetch_files_at_folder_path(config: FetchFilesAtFolderPathConfig) -> FetchFilesResult {
    STATE.with(|state| {
        state.borrow().fetch_files_at_folder_path(config)
    })
}

#[ic_cdk::query]
fn get_folder_by_id(folder_id: FolderUUID) -> Option<FolderMetadata> {
    STATE.with(|state| state.borrow().get_folder_by_id(&folder_id).cloned())
}



#[ic_cdk::query]
fn get_file_by_id(file_id: FileUUID) -> Option<FileMetadata> {
    STATE.with(|state| state.borrow().get_file_by_id(&file_id).cloned())
}


#[ic_cdk::query]
fn get_folder_by_path(path: DriveFullFilePath) -> Option<FolderMetadata> {
    STATE.with(|state| state.borrow().get_folder_by_path(&path).cloned())
}


#[ic_cdk::query]
fn get_file_by_path(path: DriveFullFilePath) -> Option<FileMetadata> {
    STATE.with(|state| state.borrow().get_file_by_path(&path).cloned())
}

#[ic_cdk::update] 
fn rename_folder(folder_id: FolderUUID, new_name: String) -> Result<(), String> {
    STATE.with(|state| {
        // Borrow the state mutably and call the method
        state.borrow_mut().rename_folder(folder_id, new_name)
    })
}

#[ic_cdk::update]
fn rename_file(file_id: FileUUID, new_name: String) -> Result<(), String> {
    STATE.with(|state| {
        state.borrow_mut().rename_file(file_id, new_name)
    })
}


#[ic_cdk::update]
fn delete_folder(folder_id: FolderUUID) -> Result<(), String> {
    STATE.with(|state| state.borrow_mut().delete_folder(&folder_id))
}

#[ic_cdk::update]
fn delete_file(file_id: FileUUID) -> Result<(), String> {
    STATE.with(|state| state.borrow_mut().delete_file(&file_id))
}

#[ic_cdk::update]
fn upsert_cloud_file_with_local_sync(file_id: FileUUID, file_metadata: FileMetadata) -> Result<(FileUUID), String> {
    STATE.with(|state| state.borrow_mut().upsert_cloud_file_with_local_sync(&file_id, &file_metadata))
}

#[ic_cdk::update]
fn upsert_cloud_folder_with_local_sync(folder_id: FolderUUID, folder_metadata: FolderMetadata) -> Result<(FolderUUID), String> {
    STATE.with(|state| state.borrow_mut().upsert_cloud_folder_with_local_sync(&folder_id, &folder_metadata))
}

#[ic_cdk::query]
fn snapshot_hashtables() -> StateSnapshot {
    STATE.with(|state| state.borrow().snapshot_hashtables())
}

#[ic_cdk::query]
fn get_canister_balance() -> u64 {
    let balance = ic_cdk::api::canister_balance();
    // print canister balance
    ic_cdk::println!("Canister balance: {}", balance);
    balance
}

#[ic_cdk::update]
fn update_username(new_username: String) -> Result<(), String> {
    STATE.with(|state| {
        state.borrow_mut().update_username(new_username)
    })
}


#[ic_cdk::query]
fn get_username() -> String {
    STATE.with(|state| state.borrow().username.clone())
}

#[ic_cdk::query]
fn get_owner() -> Principal {
    STATE.with(|state| state.borrow().owner)
}


#[derive(Clone, CandidType, Serialize, Deserialize)]
struct FetchFilesAtFolderPathConfig {
    full_folder_path: String,
    limit: u32,
    after: u32,
}

#[derive(Clone, CandidType, Serialize, Deserialize)]
struct FetchFilesResult {
    folders: Vec<FolderMetadata>,
    files: Vec<FileMetadata>,
    total: u32,
    has_more: bool,
}

impl FetchFilesResult {
    fn empty() -> Self {
        FetchFilesResult {
            folders: Vec::new(),
            files: Vec::new(),
            total: 0,
            has_more: false,
        }
    }
}

#[cfg(test)]
mod logic_tests;
