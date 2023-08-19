use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    handler::{
        blog_list_handler, create_blog_handler, delete_blog_handler, edit_blog_handler,
        get_blog_handler,
    },
    AppState,
};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/blog/new", post(create_blog_handler))
        .route("/api/blog", get(blog_list_handler))
        .route(
            "/api/blog/:id",
            get(get_blog_handler)
                .patch(edit_blog_handler)
                .delete(delete_blog_handler),
        )
        .with_state(app_state)
}
