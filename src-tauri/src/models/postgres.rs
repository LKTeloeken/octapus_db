use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreDatabase {
    pub datname: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreSchema {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreTable {
    pub name: String,
    pub schema: String,
    pub table_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreColumn {
    pub name: String,
    pub ordinal_position: i32,
    pub data_type: String,
    pub is_nullable: bool,
    pub column_default: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreTrigger {
    pub schema_name: String,
    pub table_name: String,
    pub name: String,
    pub action_timing: String,
    pub events: Vec<String>,
    pub action_statement: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreIndex {
    pub schema_name: String,
    pub table_name: String,
    pub name: String,
    pub index_def: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgrePrimaryKey {
    pub constraint_name: String,
    pub column_name: String,
    pub ordinal_position: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreForeignKey {
    pub constraint_name: String,
    pub column_name: String,
    pub table_schema: String,
    pub table_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreView {
    pub name: String,
    pub schema: String,
    pub definition: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreSequence {
    pub schema: String,
    pub name: String,
    pub data_type: String,
    pub start_value: i64,
    pub minimum_value: i64,
    pub maximum_value: i64,
    pub increment: i64,
    pub cycle_option: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostgreServer {
    pub id: Option<i32>,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub default_database: Option<String>,
    pub created_at: i64,
}