use common::{AppError, AppResult};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Member,
    Admin,
    Owner,
}

impl Role {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(Role::Owner),
            "admin" => Some(Role::Admin),
            "member" => Some(Role::Member),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActorVia {
    Session,
    ApiKey,
    OAuth,
    NoAuth,
}

/// The authenticated caller, bound to exactly one organization.
/// Roles are resolved from the membership row per request, never stored in tokens.
#[derive(Debug, Clone)]
pub struct Actor {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub role: Role,
    pub via: ActorVia,
}

impl Actor {
    pub fn require(&self, min: Role) -> AppResult<()> {
        if self.role >= min {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "this action needs the {min:?} role or higher; ask an owner of this workspace"
            )))
        }
    }
}
