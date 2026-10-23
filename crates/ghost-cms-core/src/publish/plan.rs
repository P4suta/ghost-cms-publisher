use ghost_cms_policy::{Content, Decision, Remote};

use super::PlannedAction;
use crate::domain::Post;
use crate::error::{CoreError, Result};
use crate::hash::PostState;

pub(super) enum Plan {
    Create,
    Update { id: String, updated_at: String },
    Skip { id: String },
}

impl Plan {
    pub(super) const fn action(&self) -> PlannedAction {
        match self {
            Self::Create => PlannedAction::Create,
            Self::Update { .. } => PlannedAction::Update,
            Self::Skip { .. } => PlannedAction::Skip,
        }
    }
}

pub(super) fn decide(
    existing: Option<Post>,
    content_hash: &str,
    cached: Option<&PostState>,
    force: bool,
) -> Result<Plan> {
    let remote = match existing.as_ref() {
        None => Remote::Absent,
        Some(post) if post.updated_at.as_ref().is_none_or(String::is_empty) => {
            Remote::MissingRevision
        },
        Some(post) => match cached {
            None => Remote::Unrecorded,
            Some(receipt) if receipt.id == post.id && receipt.updated_at == post.updated_at => {
                Remote::MatchesReceipt
            },
            Some(_) => Remote::Changed,
        },
    };
    let content = if cached.is_some_and(|c| c.content_hash == content_hash) {
        Content::Same
    } else {
        Content::Different
    };
    match (ghost_cms_policy::decide(remote, content, force), existing) {
        (Decision::Create, None) => Ok(Plan::Create),
        (Decision::Skip, Some(post)) => Ok(Plan::Skip { id: post.id }),
        (Decision::Update, Some(post)) => post.updated_at.map(|updated_at| Plan::Update { id: post.id, updated_at })
            .ok_or_else(|| CoreError::Inconsistent("missing remote revision".to_owned())),
        (Decision::RemoteChanged, _) => Err(CoreError::Inconsistent("remote post changed since the last publication; review it or pass --force to overwrite".to_owned())),
        (Decision::MissingRevision, _) => Err(CoreError::Inconsistent("existing post is missing updated_at; cannot update safely".to_owned())),
        (Decision::Create, Some(_)) | (Decision::Skip | Decision::Update, None) => Err(CoreError::Inconsistent("publication observation changed".to_owned())),
    }
}
