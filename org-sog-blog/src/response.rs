use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Serialize)]
pub struct GenericResponse {
    pub status: String,
    pub message: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Debug)]
pub struct BlogResponse {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub category: String,
    pub published: bool,
    pub createdAt: DateTime<Utc>,
    pub updatedAt: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct BlogData {
    pub blog: BlogResponse,
}

#[derive(Serialize, Debug)]
pub struct SingleBlogResponse {
    pub status: &'static str,
    pub data: BlogData,
}

#[derive(Serialize, Debug)]
pub struct BlogListResponse {
    pub status: &'static str,
    pub results: usize,
    pub blogs: Vec<BlogResponse>,
}
