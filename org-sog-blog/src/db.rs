use crate::error::MyError;
use crate::response::{BlogData, BlogListResponse, BlogResponse, SingleBlogResponse};
use crate::{
    error::MyError::*, model::BlogModel, schema::CreateBlogSchema, schema::UpdateBlogSchema,
};
use chrono::prelude::*;
use futures::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, Document};
use mongodb::options::{FindOneAndUpdateOptions, FindOptions, IndexOptions, ReturnDocument};
use mongodb::{bson, options::ClientOptions, Client, Collection, IndexModel};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct DB {
    pub blog_collection: Collection<BlogModel>,
    pub collection: Collection<Document>,
}

type Result<T> = std::result::Result<T, MyError>;

impl DB {
    pub async fn init() -> Result<Self> {
        let mongodb_uri = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set.");
        let database_name =
            std::env::var("MONGO_INITDB_DATABASE").expect("MONGO_INITDB_DATABASE must be set.");
        let collection_name =
            std::env::var("MONGODB_NOTE_COLLECTION").expect("MONGODB_NOTE_COLLECTION must be set.");

        let mut client_options = ClientOptions::parse(mongodb_uri).await?;
        client_options.app_name = Some(database_name.to_string());

        let client = Client::with_options(client_options)?;
        let database = client.database(database_name.as_str());

        let blog_collection = database.collection(collection_name.as_str());
        let collection = database.collection::<Document>(collection_name.as_str());

        println!("✅ Database connected successfully");

        Ok(Self {
            blog_collection,
            collection,
        })
    }

    pub async fn fetch_blogs(&self, limit: i64, page: i64) -> Result<BlogListResponse> {
        let find_options = FindOptions::builder()
            .limit(limit)
            .skip(u64::try_from((page - 1) * limit).unwrap())
            .build();

        let mut cursor = self
            .blog_collection
            .find(None, find_options)
            .await
            .map_err(MongoQueryError)?;

        let mut json_result: Vec<BlogResponse> = Vec::new();
        while let Some(doc) = cursor.next().await {
            json_result.push(self.doc_to_blog(&doc.unwrap())?);
        }

        Ok(BlogListResponse {
            status: "success",
            results: json_result.len(),
            blogs: json_result,
        })
    }

    pub async fn create_blog(&self, body: &CreateBlogSchema) -> Result<SingleBlogResponse> {
        let published = body.published.to_owned().unwrap_or(false);
        let category = body.category.to_owned().unwrap_or_default();

        let document = self.create_blog_document(body, published, category)?;

        let options = IndexOptions::builder().unique(true).build();
        let index = IndexModel::builder()
            .keys(doc! {"title": 1})
            .options(options)
            .build();

        match self.blog_collection.create_index(index, None).await {
            Ok(_) => {}
            Err(e) => return Err(MongoQueryError(e)),
        };

        let insert_result = match self.collection.insert_one(&document, None).await {
            Ok(result) => result,
            Err(e) => {
                if e.to_string()
                    .contains("E11000 duplicate key error collection")
                {
                    return Err(MongoDuplicateError(e));
                }
                return Err(MongoQueryError(e));
            }
        };

        let new_id = insert_result
            .inserted_id
            .as_object_id()
            .expect("issue with new _id");

        let blog_doc = match self
            .blog_collection
            .find_one(doc! {"_id": new_id}, None)
            .await
        {
            Ok(Some(doc)) => doc,
            Ok(None) => return Err(NotFoundError(new_id.to_string())),
            Err(e) => return Err(MongoQueryError(e)),
        };

        Ok(SingleBlogResponse {
            status: "success",
            data: BlogData {
                blog: self.doc_to_blog(&blog_doc)?,
            },
        })
    }

    pub async fn get_blog(&self, id: &str) -> Result<SingleBlogResponse> {
        let oid = ObjectId::from_str(id).map_err(|_| InvalidIDError(id.to_owned()))?;

        let blog_doc = self
            .blog_collection
            .find_one(doc! {"_id":oid }, None)
            .await
            .map_err(MongoQueryError)?;

        match blog_doc {
            Some(doc) => {
                let blog = self.doc_to_blog(&doc)?;
                Ok(SingleBlogResponse {
                    status: "success",
                    data: BlogData { blog },
                })
            }
            None => Err(NotFoundError(id.to_string())),
        }
    }

    pub async fn edit_blog(&self, id: &str, body: &UpdateBlogSchema) -> Result<SingleBlogResponse> {
        let oid = ObjectId::from_str(id).map_err(|_| InvalidIDError(id.to_owned()))?;

        let update = doc! {
            "$set": bson::to_document(body).map_err(MongoSerializeBsonError)?,
        };

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        if let Some(doc) = self
            .blog_collection
            .find_one_and_update(doc! {"_id": oid}, update, options)
            .await
            .map_err(MongoQueryError)?
        {
            let blog = self.doc_to_blog(&doc)?;
            let blog_response = SingleBlogResponse {
                status: "success",
                data: BlogData { blog },
            };
            Ok(blog_response)
        } else {
            Err(NotFoundError(id.to_string()))
        }
    }

    pub async fn delete_blog(&self, id: &str) -> Result<()> {
        let oid = ObjectId::from_str(id).map_err(|_| InvalidIDError(id.to_owned()))?;
        let filter = doc! {"_id": oid };

        let result = self
            .collection
            .delete_one(filter, None)
            .await
            .map_err(MongoQueryError)?;

        match result.deleted_count {
            0 => Err(NotFoundError(id.to_string())),
            _ => Ok(()),
        }
    }

    fn doc_to_blog(&self, blog: &BlogModel) -> Result<BlogResponse> {
        let blog_response = BlogResponse {
            id: blog.id.to_hex(),
            title: blog.title.to_owned(),
            summary: blog.summary.to_owned(),
            content: blog.content.to_owned(),
            category: blog.category.to_owned().unwrap(),
            published: blog.published.unwrap(),
            createdAt: blog.createdAt,
            updatedAt: blog.updatedAt,
        };

        Ok(blog_response)
    }

    fn create_blog_document(
        &self,
        body: &CreateBlogSchema,
        published: bool,
        category: String,
    ) -> Result<bson::Document> {
        let serialized_data = bson::to_bson(body).map_err(MongoSerializeBsonError)?;
        let document = serialized_data.as_document().unwrap();

        let datetime = Utc::now();

        let mut doc_with_dates = doc! {
            "createdAt": datetime,
            "updatedAt": datetime,
            "published": published,
            "category": category
        };
        doc_with_dates.extend(document.clone());

        Ok(doc_with_dates)
    }
}
