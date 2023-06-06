use std::path::PathBuf;

use common::state::storage::Storage;
use warp::constellation::{directory::Directory, item::Item};

use crate::layouts::storage::datasource::remote::StorageRemoteDataSource;

#[derive(Clone)]
pub struct StorageRepository {
    storageRemoteDataSource: StorageRemoteDataSource,
}

impl StorageRepository {
    pub fn new() -> Self {
        Self {
            storageRemoteDataSource: StorageRemoteDataSource::new(),
        }
    }

    pub async fn create_new_directory(
        &self,
        directory_name: String,
    ) -> Result<(), warp::error::Error> {
        self.storageRemoteDataSource
            .create_new_directory(directory_name)
            .await
    }

    pub async fn get_items_from_current_directory(&self) -> Result<Storage, warp::error::Error> {
        self.storageRemoteDataSource
            .get_items_from_current_directory()
            .await
    }

    pub async fn open_directory(
        &self,
        directory_name: String,
    ) -> Result<Storage, warp::error::Error> {
        self.storageRemoteDataSource
            .open_directory(directory_name)
            .await
    }

    pub async fn back_to_previous_directory(
        &self,
        directory: Directory,
    ) -> Result<Storage, warp::error::Error> {
        self.storageRemoteDataSource
            .back_to_previous_directory(directory)
            .await
    }

    pub async fn download_file(
        &self,
        file_name: String,
        local_path_to_save_file: PathBuf,
    ) -> Result<(), warp::error::Error> {
        self.storageRemoteDataSource
            .download_file(file_name, local_path_to_save_file)
            .await
    }

    pub async fn delete_item(&self, item: Item) -> Result<Storage, warp::error::Error> {
        self.storageRemoteDataSource.delete_items(item).await
    }
}
