use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct GenericResponse {
    pub status: String,
    pub message: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Debug)]
pub struct UserResponse {
    pub id: String,
    pub name: String,
    pub uid: String,
    pub createdAt: DateTime<Utc>,
    pub updatedAt: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct UserData {
    pub user: UserResponse,
}

#[derive(Serialize, Debug)]
pub struct SingleUserResponse {
    pub status: &'static str,
    pub data: UserData,
}

#[derive(Serialize, Debug)]
pub struct UserListResponse {
    pub status: &'static str,
    pub results: usize,
    pub users: Vec<UserResponse>,
}
