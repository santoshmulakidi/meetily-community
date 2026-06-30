//! Chat service trait and implementation

mod chat_service;

pub use chat_service::ChatServiceImpl;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::ServiceResult;