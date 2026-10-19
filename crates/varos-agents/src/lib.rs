//! Varos: agents work on the same document as the person at the keyboard, without pairing or
//! prompts, and the person sees them work.
//!
//! Pure logic, no window, renderer or UI toolkit, so everything here runs in headless tests:
//!
//! - [`actor`]: who an automation request comes from, and the label shown for it
//!   ("Claude Code", "Claude Code 2").
//! - [`diff`]: what one command changed, from the document before and after (unchanged subtrees
//!   are shared `Arc`s and are skipped).
//! - [`presence`]: the orange on-canvas feedback derived from those changes: the artboard an agent
//!   works on is outlined and the objects it creates or changes appear one by one, then fade.
//!   Nothing is sent by the agent for this; it costs no tokens.
//! - [`activity`]: a bounded per-document log of changes by revision, for `describe since` and for
//!   reviewing what each agent did.
//! - [`receipts`]: idempotent request ids per agent, so a retried edit is never applied twice.
//! - [`endpoint`]: where a running Varos listens and the per-launch secrets in a private file that
//!   let the user's own agents in, without any prompt.
//! - [`auth`]: how the proxy and the app prove they know those secrets (the app answers a hello
//!   before the proxy sends anything; the proxy's token is checked before a line is parsed).
//! - [`guard`]: the mistake guards on files agents write (never overwrite, never system folders),
//!   checked on a request's parameters and again at every file it writes.
//!
//! The desktop glue (control-channel hooks, painting, history rows) lives in
//! `crates/ui-egui/src/varos/`.

pub mod activity;
pub mod actor;
pub mod auth;
pub mod diff;
pub mod endpoint;
pub mod guard;
pub mod presence;
pub mod receipts;

pub use activity::{Activity, Since};
pub use actor::{ActorKey, Registry, sanitize_label};
pub use diff::{Change, diff, diff_counted};
pub use presence::{Frame, Presence};
pub use receipts::{Admission, ReceiptError, Receipts};

#[cfg(test)]
mod tests;
