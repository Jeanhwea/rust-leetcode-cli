//! Save bad network\'s ass.
pub mod models;
mod parser;
mod sql;
mod schemas;

use self::models::*;
use self::schemas::problems::dsl::*;
use self::sql::*;
use crate::{cfg, err::Error, plugins::LeetCode};
use diesel::prelude::*;
use serde_json::Value;
use reqwest::Error as ReqwestError;


/// sqlite connection
pub fn conn(p: String) -> SqliteConnection {
    SqliteConnection::establish(&p)
        .unwrap_or_else(|_| panic!("Error connecting to {:?}", p))
}

/// req if data not download
pub struct Cache(pub LeetCode);

impl Cache {
    /// ref to sqliteconnection
    fn conn(&self) -> SqliteConnection {
        conn(self.0.conf.storage.cache())
    }
    
    /// Clean cache
    pub fn clean(&self) -> Result<(), Error> {
        let res = std::fs::remove_file(&self.0.conf.storage.cache());
        if res.is_err() {
            let err = res.err().unwrap();
            error!("{:?}", Error::CacheError(format!("clean local cache failed -> {}", &err)));
            return Err(Error::CacheError(format!("clean local cache failed -> {}", &err)));
        }
        
        Ok(())
    }

    /// ref to download probems
    pub fn update(self) -> Result<(), Error> {
        let c = conn((&self.0.conf.storage.cache()).to_owned());
        let res = self.download_problems();
        if res.is_err() {
            return Err(res.err().unwrap());
        }
        
        for i in res.unwrap().into_iter() {
            let target = problems.filter(id.eq(i.id));
            let u_res = diesel::update(target).set(i.to_owned()).execute(&c);
            if u_res.is_err() {
                let err = u_res.err().unwrap();
                error!("{:?}", Error::CacheError(format!("Update cache failed -> {}", &err)));
                return Err(Error::CacheError(format!("Update cache failed -> {}", &err)));
            }
        }
        
        Ok(())
    }
    
    /// Download leetcode problems to db
    pub fn download_problems(self) -> Result<Vec<Problem>, Error> {
        info!("Downloading leetcode problems...");
        let mut ps: Vec<Problem> = vec![];

        for i in &self.0.conf.sys.categories.to_owned() {
            let res = self.0.clone().get_category_problems(&i);

            if res.is_err() {
                return Err(res.err().unwrap());
            }

            let json: Result<Value, ReqwestError> = res.unwrap().json();
            if json.is_err() {
                error!("{:?}", Error::DownloadError(format!("category {}", &i)));
                return Err(Error::DownloadError(format!("category {}", &i)));
            }

            // get "stat_status_pairs" from respnonse
            let res = parser::parse_problem(&mut ps, json.unwrap());
            if res.is_err() {
                error!("{:?}", Error::DownloadError(format!("category {}", &i)));
                return Err(Error::DownloadError(format!("category {}", &i)));
            }
        }

        // store problems into database
        let j = serde_json::to_string(&ps);
        if j.is_err() {
            error!("{:?}", Error::ParseError("data from cache"));
            return Err(Error::ParseError("data from cache"));
        }

        let count = self.get_problems().unwrap().len();
        if count == 0 {
            ps.sort_by(|a, b| b.id.partial_cmp(&a.id).unwrap());
            let res = diesel::insert_into(problems).values(&ps).execute(&self.conn());
            if res.is_err() {
                let err = res.err().unwrap();
                error!("{:?}", Error::CacheError(format!("Save to cache failed -> {}", &err)));
                return Err(Error::CacheError(format!("Save to cache failed -> {}", &err)));
            }
        }

        Ok(ps)
    }

    /// Get problems from cache
    ///
    /// if cache doesn't exist, request a new copy
    ///
    /// [TODO]:
    ///  1. make downloading async
    pub fn get_problems(&self) -> Result<Vec<Problem>, Error> {
        let res = problems.load::<Problem>(&self.conn());
        if res.is_err() {
            let err = res.err().unwrap();
            warn!("Select problems from cache failed -> {:?} -> try downloading", &err);
            return Err(Error::CacheError(
                format!("Select problems from cache failed -> {:?} -> try downloading", &err)
            ));
        }
        
        Ok(res.unwrap())
    }

    /// New cache
    pub fn new() -> Result<Self, Error> {
        let conf = cfg::locate();
        let c = conn(conf.storage.cache());
        let r = diesel::sql_query(CREATE_PROBLEMS_IF_NOT_EXISTS).execute(&c);
        if r.is_err() {
            let err = r.err().unwrap();
            error!("{:?}", Error::CacheError(format!("Create local cache failed -> {}", &err)));
            return Err(Error::CacheError(format!("Create local cache failed -> {}", &err)));
        }
        
        Ok(Cache(LeetCode::new()))
    }
}