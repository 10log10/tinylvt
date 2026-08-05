//! Transactional notification outbox for user-facing emails.
//!
//! Emails interface with the outside world: sends can fail and must not
//! sit inside fast DB paths. Delivery therefore gets the same shape as
//! intent work — durable state plus a draining worker
//! (`scheduler::process_notification_outbox`) — rather than spawned
//! futures with no retry story. Trigger sites enqueue in the same
//! transaction as the state change they announce; the unique
//! `dedup_key` (a legible deterministic string like
//! `capture_receipt:{intent_id}`) makes enqueue idempotent across claim
//! replays and is the once-only guarantee per event. `params` snapshots
//! the template's display inputs at enqueue, so a send is a pure
//! function of the row plus the member's current email address
//! (resolved at send time), and rows whose referents cascade-delete
//! still render.
//!
//! Delivery is at-least-once: a crash between the send and the
//! `sent_at` mark can duplicate an email, which is acceptable.

use payloads::{AuctionId, CommunityId, UserId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StoreError;
use crate::email::EmailTemplate;
use crate::time::TimeSource;
use jiff_sqlx::ToSqlx;

/// The `notification_kind` column, derived from the params variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "notification_kind", rename_all = "snake_case")]
pub(crate) enum NotificationKind {
    CardActionNeeded,
    AuthorizationExpiring,
    CaptureReceipt,
    BackingLost,
    CaptureFailed,
    CheckoutReleased,
}

/// Which recipient a member-and-stewards notice addresses: the member
/// the event is about, or a community steward (coleader/leader) copied
/// on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Audience {
    Member,
    Steward,
}

/// A notification's snapshotted template inputs. Amounts are
/// pre-formatted display strings (`payloads::format_amount`) — the
/// snapshot is the display input, not the raw value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum NotificationParams {
    /// A card authorization was declined in an automatic (off-session)
    /// context; automatic raises are paused until the member acts.
    CardActionNeeded {
        community_name: String,
        auction_id: AuctionId,
        decline_code: Option<String>,
    },
    /// A member-initiated pre-start hold was age-canceled because its
    /// card window no longer covers the auction ("authorize again when
    /// the start is announced"). Scheduled holds cancel silently — the
    /// T−24h task re-auths, and a failed re-auth notifies via
    /// `CardActionNeeded` instead.
    AuthorizationExpiring {
        community_name: String,
        auction_id: AuctionId,
        amount: String,
    },
    /// A settlement capture landed on the member's card.
    CaptureReceipt {
        community_name: String,
        auction_id: AuctionId,
        amount: String,
    },
    /// An active authorization was lost out-of-band. Always sent to the
    /// member (an active hold means they wanted it active, standing bid
    /// or not); sent to the community's coleaders/leader only when the
    /// hold was backing bids with standing commitment. `expired`
    /// selects the wording: Stripe's natural expiry of an aged-out hold
    /// (or a hold that died with a replaced account) versus a
    /// deliberate cancel outside TinyLVT (e.g. the community's Stripe
    /// dashboard).
    BackingLost {
        community_name: String,
        auction_id: AuctionId,
        amount: String,
        audience: Audience,
        expired: bool,
    },
    /// A settlement capture terminally failed — the moment a win
    /// degrades into member debt. Sent to the member and (with the
    /// steward wording) the community's coleaders/leader.
    CaptureFailed {
        community_name: String,
        auction_id: AuctionId,
        amount: String,
        audience: Audience,
    },
    /// An unsaved-card Checkout authorization completed after the
    /// auction could no longer use it (ended, or the hold's card window
    /// can't cover the deadline); the hold was released and nothing was
    /// charged. The member may have closed the tab believing they were
    /// backed, so this is always sent.
    CheckoutReleased {
        community_name: String,
        auction_id: AuctionId,
        amount: String,
    },
}

impl NotificationParams {
    pub(crate) fn kind(&self) -> NotificationKind {
        match self {
            Self::CardActionNeeded { .. } => NotificationKind::CardActionNeeded,
            Self::AuthorizationExpiring { .. } => {
                NotificationKind::AuthorizationExpiring
            }
            Self::CaptureReceipt { .. } => NotificationKind::CaptureReceipt,
            Self::BackingLost { .. } => NotificationKind::BackingLost,
            Self::CaptureFailed { .. } => NotificationKind::CaptureFailed,
            Self::CheckoutReleased { .. } => NotificationKind::CheckoutReleased,
        }
    }

    /// Render the email for these params. Pure — the drain loop calls
    /// this with the recipient's current username and the deployment
    /// base URL.
    pub(crate) fn render(
        &self,
        username: &str,
        base_url: &str,
    ) -> EmailTemplate {
        let auction_url = |id: &AuctionId| format!("{base_url}/auctions/{id}");
        match self {
            Self::CardActionNeeded {
                community_name,
                auction_id,
                decline_code,
            } => {
                let code_note = match decline_code {
                    Some(code) => format!(" (code: {code})"),
                    None => String::new(),
                };
                let url = auction_url(auction_id);
                EmailTemplate {
                    subject: format!(
                        "Action needed: card hold declined in {community_name}"
                    ),
                    html_body: format!(
                        "<p>Hi {username},</p>\
                         <p>A card authorization for an auction in \
                         <strong>{community_name}</strong> was \
                         declined{code_note}.</p>\
                         <p>Automatic card holds are paused: your proxy \
                         will keep bidding within your available balance \
                         only. To resume card-backed bidding, review your \
                         card and authorize again from the auction \
                         page:</p>\
                         <p><a href=\"{url}\">{url}</a></p>"
                    ),
                    text_body: format!(
                        "Hi {username},\n\n\
                         A card authorization for an auction in \
                         {community_name} was declined{code_note}.\n\n\
                         Automatic card holds are paused: your proxy will \
                         keep bidding within your available balance only. \
                         To resume card-backed bidding, review your card \
                         and authorize again from the auction page:\n\n\
                         {url}\n"
                    ),
                }
            }
            Self::AuthorizationExpiring {
                community_name,
                auction_id,
                amount,
            } => {
                let url = auction_url(auction_id);
                EmailTemplate {
                    subject: format!(
                        "Your card hold in {community_name} was released"
                    ),
                    html_body: format!(
                        "<p>Hi {username},</p>\
                         <p>Your {amount} card hold for an upcoming \
                         auction in <strong>{community_name}</strong> was \
                         released: the auction's start moved beyond what \
                         the hold's card window can cover.</p>\
                         <p>Authorize again once the start is \
                         announced:</p>\
                         <p><a href=\"{url}\">{url}</a></p>"
                    ),
                    text_body: format!(
                        "Hi {username},\n\n\
                         Your {amount} card hold for an upcoming auction \
                         in {community_name} was released: the auction's \
                         start moved beyond what the hold's card window \
                         can cover.\n\n\
                         Authorize again once the start is announced:\n\n\
                         {url}\n"
                    ),
                }
            }
            Self::CaptureReceipt {
                community_name,
                auction_id,
                amount,
            } => {
                let url = auction_url(auction_id);
                EmailTemplate {
                    subject: format!(
                        "Receipt: {amount} card payment in {community_name}"
                    ),
                    html_body: format!(
                        "<p>Hi {username},</p>\
                         <p>Your card was charged <strong>{amount}</strong> \
                         for your auction win in \
                         <strong>{community_name}</strong>.</p>\
                         <p>See the settlement details on the auction \
                         page:</p>\
                         <p><a href=\"{url}\">{url}</a></p>"
                    ),
                    text_body: format!(
                        "Hi {username},\n\n\
                         Your card was charged {amount} for your auction \
                         win in {community_name}.\n\n\
                         See the settlement details on the auction \
                         page:\n\n\
                         {url}\n"
                    ),
                }
            }
            Self::BackingLost {
                community_name,
                auction_id,
                amount,
                audience,
                expired,
            } => render_backing_lost(
                username,
                &auction_url(auction_id),
                community_name,
                amount,
                *audience,
                *expired,
            ),
            Self::CaptureFailed {
                community_name,
                auction_id,
                amount,
                audience,
            } => render_capture_failed(
                username,
                &auction_url(auction_id),
                community_name,
                amount,
                *audience,
            ),
            Self::CheckoutReleased {
                community_name,
                auction_id,
                amount,
            } => {
                let url = auction_url(auction_id);
                EmailTemplate {
                    subject: format!(
                        "Your {amount} card hold in {community_name} was \
                         released"
                    ),
                    html_body: format!(
                        "<p>Hi {username},</p>\
                         <p>Your {amount} card authorization for an \
                         auction in <strong>{community_name}</strong> \
                         completed after the auction could no longer use \
                         it, so the hold was released. You have not been \
                         charged.</p>\
                         <p>If you still want card backing, authorize \
                         again from the auction page:</p>\
                         <p><a href=\"{url}\">{url}</a></p>"
                    ),
                    text_body: format!(
                        "Hi {username},\n\n\
                         Your {amount} card authorization for an auction \
                         in {community_name} completed after the auction \
                         could no longer use it, so the hold was \
                         released. You have not been charged.\n\n\
                         If you still want card backing, authorize again \
                         from the auction page:\n\n\
                         {url}\n"
                    ),
                }
            }
        }
    }
}

/// The backing-lost email for one audience. Natural expiry and
/// dashboard cancel are different events; accusing a cancel that never
/// happened misdirects both member and stewards.
fn render_backing_lost(
    username: &str,
    url: &str,
    community_name: &str,
    amount: &str,
    audience: Audience,
    expired: bool,
) -> EmailTemplate {
    let (member_verb, steward_verb, cause) = if expired {
        (
            "expired",
            "expired",
            "expired: it reached the end of the card \
             network's hold window",
        )
    } else {
        (
            "was canceled",
            "was canceled outside TinyLVT",
            "canceled outside TinyLVT (for example, from \
             the community's Stripe dashboard)",
        )
    };
    match audience {
        Audience::Member => EmailTemplate {
            subject: format!(
                "Your card hold in {community_name} \
                 {member_verb}"
            ),
            html_body: format!(
                "<p>Hi {username},</p>\
                 <p>Your {amount} card hold for an auction in \
                 <strong>{community_name}</strong> was \
                 {cause}.</p>\
                 <p>Bids beyond your credit balance are no \
                 longer card-backed: any standing or future \
                 bid you win without backing becomes a \
                 balance you owe. You can authorize again \
                 from the auction page:</p>\
                 <p><a href=\"{url}\">{url}</a></p>"
            ),
            text_body: format!(
                "Hi {username},\n\n\
                 Your {amount} card hold for an auction in \
                 {community_name} was {cause}.\n\n\
                 Bids beyond your credit balance are no \
                 longer card-backed: any standing or future \
                 bid you win without backing becomes a \
                 balance you owe. You can authorize again \
                 from the auction page:\n\n\
                 {url}\n"
            ),
        },
        Audience::Steward => EmailTemplate {
            subject: format!(
                "A member's card hold in {community_name} \
                 {steward_verb}"
            ),
            html_body: format!(
                "<p>Hi {username},</p>\
                 <p>A {amount} card hold backing a member's \
                 standing bid in \
                 <strong>{community_name}</strong> was \
                 {cause}.</p>\
                 <p>Their bid stands, but if they win, any \
                 amount their balance can't cover becomes a \
                 debt to the community. Auction:</p>\
                 <p><a href=\"{url}\">{url}</a></p>"
            ),
            text_body: format!(
                "Hi {username},\n\n\
                 A {amount} card hold backing a member's \
                 standing bid in {community_name} was \
                 {cause}.\n\n\
                 Their bid stands, but if they win, any \
                 amount their balance can't cover becomes a \
                 debt to the community. Auction:\n\n\
                 {url}\n"
            ),
        },
    }
}

/// The capture-failed email for one audience.
fn render_capture_failed(
    username: &str,
    url: &str,
    community_name: &str,
    amount: &str,
    audience: Audience,
) -> EmailTemplate {
    match audience {
        Audience::Member => EmailTemplate {
            subject: format!(
                "Card payment of {amount} in \
                 {community_name} could not be collected"
            ),
            html_body: format!(
                "<p>Hi {username},</p>\
                 <p>The {amount} card payment for your \
                 auction win in \
                 <strong>{community_name}</strong> could not \
                 be collected: the card hold backing it was \
                 released before the charge went through.</p>\
                 <p>The amount now shows as a negative \
                 balance you owe the community. You can \
                 settle it from the community's currency \
                 page, or contact a community leader.</p>\
                 <p><a href=\"{url}\">{url}</a></p>"
            ),
            text_body: format!(
                "Hi {username},\n\n\
                 The {amount} card payment for your auction \
                 win in {community_name} could not be \
                 collected: the card hold backing it was \
                 released before the charge went through.\n\n\
                 The amount now shows as a negative balance \
                 you owe the community. You can settle it \
                 from the community's currency page, or \
                 contact a community leader.\n\n\
                 {url}\n"
            ),
        },
        Audience::Steward => EmailTemplate {
            subject: format!(
                "A member's {amount} card payment in \
                 {community_name} could not be collected"
            ),
            html_body: format!(
                "<p>Hi {username},</p>\
                 <p>A {amount} settlement card payment in \
                 <strong>{community_name}</strong> could not \
                 be collected: the member's card hold was \
                 released before the charge went through.</p>\
                 <p>The amount stands as a balance the \
                 member owes the community until they settle \
                 it. Auction:</p>\
                 <p><a href=\"{url}\">{url}</a></p>"
            ),
            text_body: format!(
                "Hi {username},\n\n\
                 A {amount} settlement card payment in \
                 {community_name} could not be collected: \
                 the member's card hold was released before \
                 the charge went through.\n\n\
                 The amount stands as a balance the member \
                 owes the community until they settle it. \
                 Auction:\n\n\
                 {url}\n"
            ),
        },
    }
}

/// Enqueue a notification, deduplicated on `dedup_key` — a replayed
/// claim or redelivered webhook no-ops on the conflict.
pub(crate) async fn enqueue_notification_tx(
    user_id: &UserId,
    dedup_key: &str,
    params: &NotificationParams,
    time_source: &TimeSource,
    executor: impl sqlx::PgExecutor<'_>,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO notification_outbox \
         (user_id, kind, dedup_key, params, created_at) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (dedup_key) DO NOTHING",
    )
    .bind(user_id)
    .bind(params.kind())
    .bind(dedup_key)
    .bind(sqlx::types::Json(params))
    .bind(time_source.now().to_sqlx())
    .execute(executor)
    .await?;
    Ok(())
}

/// Enqueue a notice about a member to the member and to the community's
/// stewards (coleaders and leader, excluding the member), one outbox row
/// per recipient with dedup key `{key_prefix}:{recipient id}`.
/// `member_params` is the member-view copy; `steward_params` the
/// steward-view copy, or `None` to notify the member only (e.g. the
/// backing-lost notice spares stewards when no commitment is at risk).
pub(crate) async fn fan_out_to_member_and_stewards(
    community_id: &CommunityId,
    member: &UserId,
    key_prefix: &str,
    member_params: &NotificationParams,
    steward_params: Option<&NotificationParams>,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    enqueue_notification_tx(
        member,
        &format!("{key_prefix}:{member}"),
        member_params,
        time_source,
        &mut **tx,
    )
    .await?;
    let Some(steward_params) = steward_params else {
        return Ok(());
    };
    let stewards: Vec<UserId> = sqlx::query_scalar(
        "SELECT user_id FROM community_members \
         WHERE community_id = $1 AND role IN ('coleader', 'leader') \
           AND user_id != $2",
    )
    .bind(community_id)
    .bind(member)
    .fetch_all(&mut **tx)
    .await?;
    for steward in stewards {
        enqueue_notification_tx(
            &steward,
            &format!("{key_prefix}:{steward}"),
            steward_params,
            time_source,
            &mut **tx,
        )
        .await?;
    }
    Ok(())
}

/// A claimed outbox row joined with its recipient, ready to render.
#[derive(sqlx::FromRow)]
pub(crate) struct DueNotification {
    pub(crate) params: sqlx::types::Json<NotificationParams>,
    pub(crate) email: String,
    pub(crate) username: String,
}

/// List unsent notifications past their failure backoff, lock-free —
/// the drain loop's selector (list-then-claim, like the proxy worker).
pub(crate) async fn list_due_notification_ids(
    pool: &sqlx::PgPool,
    time_source: &TimeSource,
) -> Result<Vec<Uuid>, StoreError> {
    Ok(sqlx::query_scalar(&format!(
        "SELECT id FROM notification_outbox \
         WHERE sent_at IS NULL \
           AND ( \
               failure_count = 0 \
               OR last_failed_at IS NULL \
               OR $1 > last_failed_at + {backoff} \
           ) \
         ORDER BY created_at",
        backoff = super::backoff_interval_sql("failure_count"),
    ))
    .bind(time_source.now().to_sqlx())
    .fetch_all(pool)
    .await?)
}

/// Claim one unsent row for delivery (`FOR UPDATE SKIP LOCKED` — a
/// concurrent drainer skips instead of double-sending). Holding the row
/// lock across the send is fine by the flag-row argument: the drainer
/// is the row's only writer.
pub(crate) async fn claim_due_notification_tx(
    id: &Uuid,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<Option<DueNotification>, StoreError> {
    Ok(sqlx::query_as(
        "SELECT n.params, u.email, u.username \
         FROM notification_outbox n \
         JOIN users u ON n.user_id = u.id \
         WHERE n.id = $1 AND n.sent_at IS NULL \
         FOR UPDATE OF n SKIP LOCKED",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?)
}

pub(crate) async fn mark_notification_sent_tx(
    id: &Uuid,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE notification_outbox \
         SET sent_at = $1, failure_count = 0, last_failed_at = NULL \
         WHERE id = $2",
    )
    .bind(time_source.now().to_sqlx())
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn record_notification_failure_tx(
    id: &Uuid,
    time_source: &TimeSource,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE notification_outbox \
         SET failure_count = failure_count + 1, last_failed_at = $1 \
         WHERE id = $2",
    )
    .bind(time_source.now().to_sqlx())
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
