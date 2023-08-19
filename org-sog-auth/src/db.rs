use crate::error::MyError;
use crate::response::{SingleUserResponse, UserData, UserListResponse, UserResponse};
use crate::{
    error::MyError::*, model::UserModel, schema::CreateUserSchema, schema::UpdateUserSchema,
};
use chrono::prelude::*;
use futures::StreamExt;
use mongodb::bson::{doc, oid::ObjectId, Document};
use mongodb::options::{FindOneAndUpdateOptions, FindOptions, IndexOptions, ReturnDocument};
use mongodb::{bson, options::ClientOptions, Client, Collection, IndexModel};
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct DB {
    pub user_collection: Collection<UserModel>,
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

        let user_collection = database.collection(collection_name.as_str());
        let collection = database.collection::<Document>(collection_name.as_str());

        println!("✅ Database connected successfully");

        Ok(Self {
            user_collection,
            collection,
        })
    }

    pub async fn fetch_users(&self, limit: i64, page: i64) -> Result<UserListResponse> {
        let find_options = FindOptions::builder()
            .limit(limit)
            .skip(u64::try_from((page - 1) * limit).unwrap())
            .build();

        let mut cursor = self
            .user_collection
            .find(None, find_options)
            .await
            .map_err(MongoQueryError)?;

        let mut json_result: Vec<UserResponse> = Vec::new();
        while let Some(doc) = cursor.next().await {
            json_result.push(self.doc_to_user(&doc.unwrap())?);
        }

        Ok(UserListResponse {
            status: "success",
            results: json_result.len(),
            users: json_result,
        })
    }

    pub async fn create_user(&self, body: &CreateUserSchema) -> Result<SingleUserResponse> {
        let document = self.create_user_document(body)?;

        let options = IndexOptions::builder().unique(true).build();
        let index = IndexModel::builder()
            .keys(doc! {"name": 1})
            .options(options)
            .build();

        match self.user_collection.create_index(index, None).await {
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

        let user_doc = match self
            .user_collection
            .find_one(doc! {"_id": new_id}, None)
            .await
        {
            Ok(Some(doc)) => doc,
            Ok(None) => return Err(NotFoundError(new_id.to_string())),
            Err(e) => return Err(MongoQueryError(e)),
        };

        Ok(SingleUserResponse {
            status: "success",
            data: UserData {
                user: self.doc_to_user(&user_doc)?,
            },
        })
    }

    pub async fn get_user(&self, id: &str) -> Result<SingleUserResponse> {
        let oid = ObjectId::from_str(id).map_err(|_| InvalidIDError(id.to_owned()))?;

        let user_doc = self
            .user_collection
            .find_one(doc! {"_id":oid }, None)
            .await
            .map_err(MongoQueryError)?;

        match user_doc {
            Some(doc) => {
                let user = self.doc_to_user(&doc)?;
                Ok(SingleUserResponse {
                    status: "success",
                    data: UserData { user },
                })
            }
            None => Err(NotFoundError(id.to_string())),
        }
    }

    pub async fn edit_user(&self, id: &str, body: &UpdateUserSchema) -> Result<SingleUserResponse> {
        let oid = ObjectId::from_str(id).map_err(|_| InvalidIDError(id.to_owned()))?;

        let update = doc! {
            "$set": bson::to_document(body).map_err(MongoSerializeBsonError)?,
        };

        let options = FindOneAndUpdateOptions::builder()
            .return_document(ReturnDocument::After)
            .build();

        if let Some(doc) = self
            .user_collection
            .find_one_and_update(doc! {"_id": oid}, update, options)
            .await
            .map_err(MongoQueryError)?
        {
            let user = self.doc_to_user(&doc)?;
            let user_response = SingleUserResponse {
                status: "success",
                data: UserData { user },
            };
            Ok(user_response)
        } else {
            Err(NotFoundError(id.to_string()))
        }
    }

    pub async fn delete_user(&self, id: &str) -> Result<()> {
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

    fn doc_to_user(&self, user: &UserModel) -> Result<UserResponse> {
        let user_response = UserResponse {
            id: user.id.to_hex(),
            name: user.name.to_owned(),
            uid: user.uid.to_owned(),
            createdAt: user.createdAt,
            updatedAt: user.updatedAt,
        };

        Ok(user_response)
    }

    fn create_user_document(&self, body: &CreateUserSchema) -> Result<bson::Document> {
        let serialized_data = bson::to_bson(body).map_err(MongoSerializeBsonError)?;
        let document = serialized_data.as_document().unwrap();

        let datetime = Utc::now();

        let mut doc_with_dates = doc! {
            "createdAt": datetime,
            "updatedAt": datetime
        };
        doc_with_dates.extend(document.clone());

        Ok(doc_with_dates)
    }
}
