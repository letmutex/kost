use std::time::Instant;

use tower_lsp::lsp_types::Url;

pub fn format_origin(url: &Url) -> String {
    let s = url.as_str();
    if let Some(after_jar) = s.strip_prefix("jar:") {
        let archive = after_jar
            .split("!/")
            .next()
            .and_then(|u| u.rsplit('/').next())
            .unwrap_or("archive.jar");
        let entry = after_jar.rsplit('/').next().unwrap_or(after_jar);
        format!("[from `{}` in `{}`]", entry, archive)
    } else if let Some(file_name) = url.path_segments().and_then(|mut s| s.next_back()) {
        format!("[from `{}`]", file_name)
    } else {
        format!("[from `{}`]", s)
    }
}

#[derive(Debug, Clone)]
pub struct TraceSpan {
    pub phase: String,
    pub target: String,
    pub steps: Vec<String>,
    pub bindings: Vec<(String, String)>,
    pub result: Option<String>,
    pub children: Vec<TraceSpan>,
}

#[derive(Debug)]
pub struct TraceCollector {
    start_time: Instant,
    root_spans: Vec<TraceSpan>,
    open_stack: Vec<TraceSpan>,
    event_count: usize,
    max_events: usize,
    dropped_scopes: usize,
}

impl Default for TraceCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceCollector {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            root_spans: Vec::with_capacity(4),
            open_stack: Vec::with_capacity(4),
            event_count: 0,
            max_events: 64,
            dropped_scopes: 0,
        }
    }

    pub fn enter_scope(&mut self, phase: &str, target: &str) {
        if self.dropped_scopes > 0 || self.event_count >= self.max_events {
            self.dropped_scopes += 1;
            return;
        }
        self.event_count += 1;
        self.open_stack.push(TraceSpan {
            phase: phase.to_string(),
            target: target.to_string(),
            steps: Vec::new(),
            bindings: Vec::new(),
            result: None,
            children: Vec::new(),
        });
    }

    pub fn record_step(&mut self, step: String) {
        if self.event_count >= self.max_events {
            return;
        }
        self.event_count += 1;
        if let Some(current) = self.open_stack.last_mut() {
            current.steps.push(step);
        }
    }

    pub fn record_binding(&mut self, param: &str, bound: &str) {
        if self.event_count >= self.max_events {
            return;
        }
        self.event_count += 1;
        if let Some(current) = self.open_stack.last_mut() {
            current
                .bindings
                .push((param.to_string(), bound.to_string()));
        }
    }

    pub fn exit_scope(&mut self, result: Option<&str>) {
        if self.dropped_scopes > 0 {
            self.dropped_scopes -= 1;
            return;
        }
        if let Some(mut span) = self.open_stack.pop() {
            span.result = result.map(|s| s.to_string());
            if let Some(parent) = self.open_stack.last_mut() {
                parent.children.push(span);
            } else {
                self.root_spans.push(span);
            }
        }
    }

    pub fn record_binding_type(&mut self, param: &str, bound: &super::types::KotlinType) {
        self.record_binding(param, &bound.to_string());
    }

    pub fn render_markdown(&self) -> String {
        if self.root_spans.is_empty() {
            return String::new();
        }

        let elapsed = self.start_time.elapsed();
        let elapsed_str = if elapsed.as_millis() > 0 {
            format!("{:.2} ms", elapsed.as_secs_f64() * 1000.0)
        } else {
            format!("{:.1} us", elapsed.as_micros() as f64)
        };

        let mut md = String::new();
        md.push_str(&format!(
            "\n\n### Type Inference Trace ({})\n\n",
            elapsed_str
        ));

        for root in &self.root_spans {
            Self::render_span(&mut md, root, 0);
        }

        md
    }

    fn render_span(md: &mut String, span: &TraceSpan, indent: usize) {
        let prefix = "  ".repeat(indent);
        let outcome = span.result.as_deref().unwrap_or("?");
        md.push_str(&format!(
            "{}* {}: `{}` -> `{}`\n",
            prefix, span.phase, span.target, outcome
        ));
        for step in &span.steps {
            md.push_str(&format!("{}  - {}\n", prefix, step));
        }
        for (param, bound) in &span.bindings {
            md.push_str(&format!("{}  - Bound `{}` = `{}`\n", prefix, param, bound));
        }
        for child in &span.children {
            Self::render_span(md, child, indent + 1);
        }
    }
}

pub enum Tracer<'a> {
    Noop,
    Active(&'a mut TraceCollector),
}

impl<'a> Tracer<'a> {
    #[inline(always)]
    pub fn is_active(&self) -> bool {
        matches!(self, Tracer::Active(_))
    }

    #[inline(always)]
    pub fn enter_scope(&mut self, phase: &str, target: &str) {
        if let Tracer::Active(c) = self {
            c.enter_scope(phase, target);
        }
    }

    #[inline(always)]
    pub fn record_step(&mut self, f: impl FnOnce() -> String) {
        if let Tracer::Active(c) = self {
            c.record_step(f());
        }
    }

    #[inline(always)]
    pub fn record_binding(&mut self, param: &str, bound: &str) {
        if let Tracer::Active(c) = self {
            c.record_binding(param, bound);
        }
    }

    #[inline(always)]
    pub fn record_binding_type(&mut self, param: &str, bound: &super::types::KotlinType) {
        if let Tracer::Active(c) = self {
            c.record_binding_type(param, bound);
        }
    }

    #[inline(always)]
    pub fn exit_scope(&mut self, result: Option<&str>) {
        if let Tracer::Active(c) = self {
            c.exit_scope(result);
        }
    }

    #[inline(always)]
    pub fn exit_scope_type(&mut self, result: Option<&super::types::KotlinType>) {
        if let Tracer::Active(c) = self {
            let formatted = result.map(|t| t.to_string());
            c.exit_scope(formatted.as_deref());
        }
    }
}
