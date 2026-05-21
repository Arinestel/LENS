#[derive(Debug, Clone, PartialEq)]
pub struct UserQuery {
    pub text: String,
    pub language: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreSessionContext {
    pub session_id: Option<String>,
    pub request_language: String,
    pub branch_id: Option<String>,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreInputEnvelope {
    pub query: UserQuery,
    pub session_context: CoreSessionContext,
}

impl CoreSessionContext {
    pub fn from_query(query: &UserQuery) -> Self {
        Self {
            session_id: query.session_id.clone(),
            request_language: query.language.clone(),
            branch_id: None,
            user_id: None,
        }
    }
}

impl From<UserQuery> for CoreInputEnvelope {
    fn from(query: UserQuery) -> Self {
        let session_context = CoreSessionContext::from_query(&query);

        Self {
            query,
            session_context,
        }
    }
}
