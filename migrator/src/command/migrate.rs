use anyhow::{anyhow, Context};
use std::{env, fs};
use std::fmt::Debug;
use std::path::PathBuf;
use aws_sdk_dynamodb::model::AttributeValue;
use serde::Deserialize;
use thiserror::__private::PathAsDisplay;

use crate::command::{ExitCode, Output};
use crate::clients::client::{Client, ExistsTableResultType};
use crate::command::migrate_operation_type::MigrateOperationType;
use crate::command::migrate_type::MigrateType;
use crate::command::query::create_table::CreateTableQuery;
use crate::command::query::delete_table::DeleteTableQuery;
use crate::command::query::get_item::{GetItemQuery, Key};
use crate::settings::Settings;

const RESOURCE_FILE_DIR: &str = "resource";
const DEFAULT_MIGRATION_FILE_PATH: &str = "migrations";

#[derive(Debug, Copy, Clone)]
pub struct Migrate {
}

impl Migrate {
    pub fn new() -> Self {
        Self {}
    }

    fn read_migration_files(&self, current_path: PathBuf) -> anyhow::Result<Vec<PathBuf>> {
        let directories = fs::read_dir(&current_path).context(format!("Cannot resolve path. File: {} ", current_path.as_display()))?;

        let mut migration_files= vec![];
        for directory in directories {
            migration_files.push(directory.context(format!("Cannot resolve path."))?.path());
        }

        Ok(migration_files)
    }

    async fn create_migration_table_for_dynamodb(self) -> anyhow::Result<()> {
        for migration_file in self.read_migration_files(self.migration_dir()?).context("")? {
            let data = std::fs::File::open(&migration_file).context("Cannot read migration file.")?;

            let query = self.from_json_file::<CreateTableQuery>(data)?;

            if ExistsTableResultType::NotFound == Client::new().exists_table(query.table_name()).await.context("Cannot check exists table.")? {
                Client::new().create_table(query.table_name(), &query).await.context("Cannot create table. {}")?;
            }
        }

        Ok(())
    }

    pub async fn execute(self, _command: &MigrateType, migrate_path: Option<&PathBuf>) -> anyhow::Result<Output> {
        let _ = Settings::new().map_err(|error| anyhow!(error))?;

        self.create_migration_table_for_dynamodb()
            .await
            .map_err(|error| anyhow!(format!("Failed create default migration table. Error: {}", error.to_string())))?;

        let path = self
            .migrate_path_resolver()(migrate_path, PathBuf::from(DEFAULT_MIGRATION_FILE_PATH));

        self.migrate(path)
            .await
            .map_err(|error| anyhow!(format!("Failed user migration data. Error: {}", error.to_string())))?;

        Ok(Output::new(ExitCode::SUCCEED, "All migrate succeed.".to_string()))
    }

    async fn migrate(self, target_path: PathBuf) -> anyhow::Result<()> {
        let files = self.read_migration_files(target_path).context("Cannot read migration file.")?;

        for file in files {
            let operation_type = MigrateOperationType::resolve(&file)?;

            let file_name = file
                .file_name()
                .context(format!("Cannot get filename from PathBuf. {:?}", file))?
                .to_string_lossy()
                .to_string();

            let query = GetItemQuery::new(
                "migrations".to_string(),
                Key::new("FileName".to_string(), AttributeValue::S(file_name.clone())),
                true
            );

            match (Client::new().get_item(&query).await?.item(), operation_type) {
                (Some(_records), _) => {
                    println!("File name {} was already executed. This file was skipped.", file_name)
                },
                (None, MigrateOperationType::CreateTable) => {
                    let data = std::fs::File::open(&file).context(format!("Cannot open migration file. FileName: {}", file_name))?;

                    let query = self.from_json_file::<CreateTableQuery>(data)?;

                    Client::new().create_table(query.table_name(), &query).await?;
                    Client::new().add_migration_record(&file).await?;
                },
                (None, MigrateOperationType::DeleteTable) => {
                    let data = std::fs::File::open(&file).context(format!("Cannot open migration file. FileName: {}", file_name))?;

                    let query = self.from_json_file::<DeleteTableQuery>(data)?;

                    Client::new().delete_table(&query).await.context("Cannot delete table. {}")?;
                    Client::new().add_migration_record(&file).await?;
                }
                (_, _) => { println!("File name {} was skipped. Unsupported command.", file_name) }
            }
        }

        Ok(())
    }

    fn from_json_file<T: for<'a> Deserialize<'a>>(self, file: std::fs::File) -> anyhow::Result<T> {
        let result: T = serde_json::from_reader(file).context("Cannot parse json file.")?;

        return Ok(result)
    }

    fn migration_dir(self) -> anyhow::Result<PathBuf> {
        Ok(env::current_dir()
            .context("Cannot find current_dir.")?
            .join("src")
            .join(RESOURCE_FILE_DIR))
    }

    fn migrate_path_resolver(self) -> fn(migrate_path: Option<&PathBuf>, default: PathBuf) -> PathBuf {
        |migrate_path, default|
            match migrate_path {
                Some(path) => path.to_path_buf(),
                _ => default,
            }
    }
}
