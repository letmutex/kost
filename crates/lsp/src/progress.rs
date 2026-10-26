use std::sync::atomic::{AtomicU64, Ordering};
use tower_lsp::Client;
use tower_lsp::lsp_types::{
    NumberOrString, ProgressParams, ProgressParamsValue, WorkDoneProgress, WorkDoneProgressBegin,
    WorkDoneProgressCreateParams, WorkDoneProgressEnd, WorkDoneProgressReport,
    notification::Progress, request::WorkDoneProgressCreate,
};

use common::concat_str;

static PROGRESS_TOKEN_COUNTER: AtomicU64 = AtomicU64::new(1);

#[inline]
fn calculate_percentage(current: usize, total: usize) -> Option<u32> {
    if total > 0 {
        let clamped = current.min(total);
        Some(((clamped as f64 / total as f64) * 100.0) as u32)
    } else {
        None
    }
}

pub struct ProgressReporter {
    client: Option<Client>,
    token: String,
    started: bool,
}

impl ProgressReporter {
    pub fn new(client: Client) -> Self {
        let id_str = PROGRESS_TOKEN_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .to_string();
        let token = concat_str!("idx-", &id_str);
        Self {
            client: Some(client),
            token,
            started: false,
        }
    }

    pub fn noop() -> Self {
        Self {
            client: None,
            token: String::new(),
            started: false,
        }
    }

    pub async fn begin(&mut self, title: &str) {
        if self.started {
            return;
        }
        let Some(client) = &self.client else { return };
        let params = WorkDoneProgressCreateParams {
            token: NumberOrString::String(self.token.clone()),
        };

        if client
            .send_request::<WorkDoneProgressCreate>(params)
            .await
            .is_err()
        {
            self.client = None;
            return;
        }

        self.started = true;
        client
            .send_notification::<Progress>(ProgressParams {
                token: NumberOrString::String(self.token.clone()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: title.to_string(),
                        cancellable: Some(false),
                        message: None,
                        percentage: Some(0),
                    },
                )),
            })
            .await;
    }

    pub fn is_active(&self) -> bool {
        self.client.is_some() && self.started
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub async fn report(&self, message: &str, current: usize, total: usize) {
        if !self.started {
            return;
        }
        let percentage = calculate_percentage(current, total);
        self.send_report(message, percentage).await;
    }

    pub async fn report_pct(&self, message: &str, percentage: u32) {
        if !self.started {
            return;
        }
        self.send_report(message, Some(percentage.min(100))).await;
    }

    async fn send_report(&self, message: &str, percentage: Option<u32>) {
        let Some(client) = &self.client else { return };

        client
            .send_notification::<Progress>(ProgressParams {
                token: NumberOrString::String(self.token.clone()),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                    WorkDoneProgressReport {
                        cancellable: None,
                        message: Some(message.to_string()),
                        percentage,
                    },
                )),
            })
            .await;
    }

    pub async fn finish(mut self, message: &str) {
        if !self.started {
            return;
        }
        self.started = false;
        if let Some(client) = self.client.take() {
            client
                .send_notification::<Progress>(ProgressParams {
                    token: NumberOrString::String(self.token.clone()),
                    value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(
                        WorkDoneProgressEnd {
                            message: Some(message.to_string()),
                        },
                    )),
                })
                .await;
        }
    }
}

impl Drop for ProgressReporter {
    fn drop(&mut self) {
        if !self.started {
            return;
        }
        if let Some(client) = self.client.take() {
            let token = std::mem::take(&mut self.token);
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    client
                        .send_notification::<Progress>(ProgressParams {
                            token: NumberOrString::String(token),
                            value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(
                                WorkDoneProgressEnd { message: None },
                            )),
                        })
                        .await;
                });
            }
        }
    }
}
