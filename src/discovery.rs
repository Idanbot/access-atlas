use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Kubernetes,
    Aws,
    Gcloud,
    Azure,
    Terraform,
    Ssh,
    Docker,
    Tailscale,
    Cloudflare,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kubernetes => "kubernetes",
            Self::Aws => "aws",
            Self::Gcloud => "gcloud",
            Self::Azure => "azure",
            Self::Terraform => "terraform",
            Self::Ssh => "ssh",
            Self::Docker => "docker",
            Self::Tailscale => "tailscale",
            Self::Cloudflare => "cloudflare",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActionKind {
    Connect,
    PortForward,
    Debug,
    Inspect,
    Logs,
    Shell,
    Copy,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandTemplate {
    pub id: String,
    pub label: String,
    pub kind: ActionKind,
    pub command: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredConnection {
    pub id: String,
    pub label: String,
    pub provider: Provider,
    pub kind: String,
    pub metadata: BTreeMap<String, String>,
    pub commands: Vec<CommandTemplate>,
}

impl DiscoveredConnection {
    pub fn primary_commands(&self) -> Vec<&CommandTemplate> {
        let mut selected = Vec::with_capacity(3);
        let mut ids = BTreeSet::new();

        for kind in [
            ActionKind::Connect,
            ActionKind::PortForward,
            ActionKind::Debug,
        ] {
            if let Some(command) = self.commands.iter().find(|command| command.kind == kind) {
                ids.insert(command.id.as_str());
                selected.push(command);
            }
        }
        for command in &self.commands {
            if selected.len() == 3 {
                break;
            }
            if ids.insert(command.id.as_str()) {
                selected.push(command);
            }
        }
        selected
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionInventory {
    pub schema_version: u32,
    pub generated_at_unix: u64,
    pub connections: Vec<DiscoveredConnection>,
}

impl Default for ConnectionInventory {
    fn default() -> Self {
        Self {
            schema_version: 1,
            generated_at_unix: 0,
            connections: Vec::new(),
        }
    }
}

impl ConnectionInventory {
    pub fn deduplicate(&mut self) {
        self.connections
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.connections.dedup_by(|left, right| left.id == right.id);
    }

    pub fn merge(mut cached: Self, fresh: Self) -> Self {
        let mut updates = fresh
            .connections
            .into_iter()
            .map(|connection| (connection.id.clone(), connection))
            .collect::<BTreeMap<_, _>>();
        for connection in &mut cached.connections {
            if let Some(update) = updates.remove(&connection.id) {
                *connection = update;
            }
        }
        cached.connections.extend(updates.into_values());
        cached.deduplicate();
        cached.generated_at_unix = cached.generated_at_unix.max(fresh.generated_at_unix);
        cached.schema_version = cached.schema_version.max(fresh.schema_version);
        cached
    }

    pub fn is_stale_at(&self, now_unix: u64, max_age: Duration) -> bool {
        !self.connections.is_empty()
            && now_unix.saturating_sub(self.generated_at_unix) > max_age.as_secs()
    }

    pub fn reconcile(
        mut cached: Self,
        fresh: Self,
        sources: &[SourceReport],
        mode: DiscoveryMode,
    ) -> Self {
        let authoritative = sources
            .iter()
            .filter(|source| source.state == SourceState::Loaded)
            .map(|source| source.provider)
            .collect::<BTreeSet<_>>();
        cached.connections.retain(|connection| {
            !authoritative.contains(&connection.provider)
                || (mode == DiscoveryMode::Local && is_online_resource(&connection.kind))
        });
        cached.connections.extend(fresh.connections);
        cached.deduplicate();
        if !authoritative.is_empty() {
            cached.generated_at_unix = fresh.generated_at_unix;
        }
        cached.schema_version = cached.schema_version.max(fresh.schema_version);
        cached
    }
}

fn is_online_resource(kind: &str) -> bool {
    matches!(
        kind,
        "ec2-instance" | "compute-instance" | "virtual-machine"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryMode {
    Local,
    Online,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceState {
    Loaded,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceReport {
    pub provider: Provider,
    pub state: SourceState,
    pub connections: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshReport {
    pub inventory: ConnectionInventory,
    pub sources: Vec<SourceReport>,
    pub notices: Vec<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryEvent {
    Started { total: usize },
    Source(SourceReport),
    Finished { completed: usize, cancelled: bool },
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceReport {
    pub passed: bool,
    pub connection_count: usize,
    pub command_count: usize,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn audit_refresh(refresh: &RefreshReport) -> AcceptanceReport {
    let mut issues = Vec::new();
    let mut warnings = refresh.notices.clone();
    let mut connection_ids = BTreeSet::new();
    let mut command_count = 0;

    if refresh.inventory.connections.is_empty() {
        issues.push("no connections were discovered".to_owned());
    }
    for source in &refresh.sources {
        if source.state != SourceState::Loaded {
            warnings.push(format!(
                "{} {}: {}",
                source.provider.as_str(),
                format!("{:?}", source.state).to_lowercase(),
                source.message
            ));
        }
    }
    for connection in &refresh.inventory.connections {
        if !connection_ids.insert(connection.id.as_str()) {
            issues.push(format!("duplicate connection id: {}", connection.id));
        }
        if connection.commands.len() != 10 {
            issues.push(format!(
                "{} has {} commands; expected exactly 10",
                connection.id,
                connection.commands.len()
            ));
        }
        let mut command_ids = BTreeSet::new();
        for command in &connection.commands {
            command_count += 1;
            if !command_ids.insert(command.id.as_str()) {
                issues.push(format!(
                    "{} has duplicate command id {}",
                    connection.id, command.id
                ));
            }
            if command.command.trim().is_empty() {
                issues.push(format!(
                    "{}/{} has an empty command",
                    connection.id, command.id
                ));
            }
            if command.command.chars().any(|character| {
                character == '\n'
                    || character == '\r'
                    || character == '\0'
                    || character.is_control()
            }) {
                issues.push(format!(
                    "{}/{} command must be a single line without control characters",
                    connection.id, command.id
                ));
            }
            if command.command.contains('{') || command.command.contains('}') {
                issues.push(format!(
                    "{}/{} contains an unresolved template placeholder",
                    connection.id, command.id
                ));
            }
        }
        for key in connection.metadata.keys() {
            let normalized = key.to_ascii_lowercase();
            if [
                "secret",
                "token",
                "credential",
                "identity_file",
                "private_key",
            ]
            .iter()
            .any(|sensitive| normalized.contains(sensitive))
            {
                issues.push(format!(
                    "{} exposes prohibited credential metadata key {key}",
                    connection.id
                ));
            }
        }
    }

    AcceptanceReport {
        passed: issues.is_empty(),
        connection_count: refresh.inventory.connections.len(),
        command_count,
        issues,
        warnings,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
    pub program: String,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
}

impl CommandRequest {
    fn new(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            current_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            status: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.status == 0
    }
}

pub trait CommandRunner: Send + Sync {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult>;
}

#[derive(Debug, Clone)]
pub struct ProcessRunner {
    timeout: Duration,
    search_path: Option<Vec<PathBuf>>,
}

impl ProcessRunner {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            search_path: None,
        }
    }

    pub fn with_search_path(mut self, search_path: Vec<PathBuf>) -> Self {
        self.search_path = Some(search_path);
        self
    }

    fn executable(&self, program: &str) -> io::Result<PathBuf> {
        let Some(search_path) = &self.search_path else {
            return Ok(PathBuf::from(program));
        };
        search_path
            .iter()
            .map(|directory| directory.join(program))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{program} is not installed"),
                )
            })
    }
}

impl CommandRunner for ProcessRunner {
    fn run(&self, request: &CommandRequest) -> io::Result<CommandResult> {
        let executable = self.executable(&request.program)?;
        let mut command = Command::new(executable);
        command
            .args(&request.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(current_dir) = &request.current_dir {
            command.current_dir(current_dir);
        }
        if let Some(search_path) = &self.search_path {
            let joined = env::join_paths(search_path).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid PATH: {error}"),
                )
            })?;
            command.env("PATH", joined);
        }

        let mut child = command.spawn()?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("command stdout was not captured"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("command stderr was not captured"))?;
        let stdout_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let stderr_reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("{} exceeded {:?}", request.program, self.timeout),
                ));
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = join_reader(stdout_reader, "stdout")?;
        let stderr = join_reader(stderr_reader, "stderr")?;
        Ok(CommandResult {
            status: status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
}

fn join_reader(
    reader: thread::JoinHandle<io::Result<Vec<u8>>>,
    stream: &str,
) -> io::Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| io::Error::other(format!("{stream} reader panicked")))?
}

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub home: PathBuf,
    pub terraform_roots: Vec<PathBuf>,
    pub template_overrides: Option<PathBuf>,
}

impl DiscoveryConfig {
    pub fn new(home: PathBuf, terraform_roots: Vec<PathBuf>) -> Self {
        Self {
            home,
            terraform_roots,
            template_overrides: None,
        }
    }

    pub fn with_template_overrides(mut self, path: PathBuf) -> Self {
        self.template_overrides = Some(path);
        self
    }
}

#[derive(Debug, Clone)]
pub struct InventoryCache {
    path: PathBuf,
}

impl InventoryCache {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn load_or_default(&self) -> io::Result<ConnectionInventory> {
        let input = match fs::read_to_string(&self.path) {
            Ok(input) => input,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ConnectionInventory::default());
            }
            Err(error) => return Err(error),
        };
        serde_json::from_str(&input).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("parse inventory cache {}: {error}", self.path.display()),
            )
        })
    }

    pub fn store(&self, inventory: &ConnectionInventory) -> io::Result<()> {
        use std::io::Write;

        let parent = self.path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "cache path has no parent")
        })?;
        fs::create_dir_all(parent)?;
        let filename = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("connections.json");
        let temporary = parent.join(format!(".{filename}.tmp-{}", std::process::id()));
        let json = serde_json::to_vec_pretty(inventory).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("serialize inventory cache: {error}"),
            )
        })?;
        let mut output = fs::File::create(&temporary)?;
        output.write_all(&json)?;
        output.sync_all()?;
        drop(output);
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        Ok(())
    }
}

pub struct DiscoveryService<R> {
    runner: R,
    config: DiscoveryConfig,
}

impl<R: CommandRunner> DiscoveryService<R> {
    pub fn new(runner: R, config: DiscoveryConfig) -> Self {
        Self { runner, config }
    }

    pub fn refresh(&self, mode: DiscoveryMode) -> RefreshReport {
        self.refresh_with_progress(mode, &CancellationToken::new(), |_| {})
    }

    pub fn refresh_with_progress<F>(
        &self,
        mode: DiscoveryMode,
        cancellation: &CancellationToken,
        mut progress: F,
    ) -> RefreshReport
    where
        F: FnMut(DiscoveryEvent),
    {
        const SOURCE_COUNT: usize = 9;
        progress(DiscoveryEvent::Started {
            total: SOURCE_COUNT,
        });
        let mut connections = Vec::new();
        let mut sources = Vec::new();

        macro_rules! scan {
            ($discovery:expr) => {
                if !cancellation.is_cancelled() {
                    let (found, source) = $discovery;
                    connections.extend(found);
                    progress(DiscoveryEvent::Source(source.clone()));
                    sources.push(source);
                }
            };
        }
        scan!(discover_kubernetes(&self.runner));
        scan!(discover_aws(&self.runner, mode));
        scan!(discover_gcloud(&self.runner, mode));
        scan!(discover_azure(&self.runner, mode));
        scan!(discover_terraform(&self.config));
        scan!(discover_ssh(&self.config));
        scan!(discover_docker(&self.runner));
        scan!(discover_tailscale(&self.runner));
        scan!(discover_cloudflare(&self.config));

        let cancelled = cancellation.is_cancelled();
        let mut inventory = ConnectionInventory {
            schema_version: 1,
            generated_at_unix: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs()),
            connections,
        };
        inventory.deduplicate();
        let notices = self
            .config
            .template_overrides
            .as_deref()
            .map_or_else(Vec::new, |path| {
                apply_template_overrides(&mut inventory, path)
            });
        let completed = sources.len();
        progress(DiscoveryEvent::Finished {
            completed,
            cancelled,
        });
        RefreshReport {
            inventory,
            sources,
            notices,
            cancelled,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TemplateOverrideFile {
    version: u32,
    #[serde(default)]
    overrides: Vec<TemplateOverride>,
}

#[derive(Debug, Deserialize)]
struct TemplateOverride {
    provider: Provider,
    resource_kind: String,
    id: String,
    label: String,
    action: ActionKind,
    command: String,
    description: String,
    #[serde(default)]
    position: Option<usize>,
}

fn apply_template_overrides(
    inventory: &mut ConnectionInventory,
    path: &std::path::Path,
) -> Vec<String> {
    let input = match fs::read_to_string(path) {
        Ok(input) => input,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            return vec![format!(
                "read template overrides {}: {error}",
                path.display()
            )];
        }
    };
    let file: TemplateOverrideFile = match serde_json::from_str(&input) {
        Ok(file) => file,
        Err(error) => {
            return vec![format!(
                "parse template overrides {}: {error}",
                path.display()
            )];
        }
    };
    if file.version != 1 {
        return vec![format!(
            "template overrides {} use unsupported version {}",
            path.display(),
            file.version
        )];
    }

    let mut notices = Vec::new();
    for connection in &mut inventory.connections {
        for entry in file.overrides.iter().filter(|entry| {
            entry.provider == connection.provider && entry.resource_kind == connection.kind
        }) {
            match rendered_override(entry, connection) {
                Ok(command) => {
                    if let Some(index) = connection
                        .commands
                        .iter()
                        .position(|existing| existing.id == entry.id)
                    {
                        connection.commands[index] = command;
                    } else if let Some(position) = entry.position.filter(|position| *position > 0) {
                        let index = (position - 1).min(connection.commands.len());
                        connection.commands.insert(index, command);
                        connection.commands.truncate(10);
                    } else {
                        notices.push(format!(
                            "override {} for {}/{} does not replace a built-in command and needs position 1..10",
                            entry.id,
                            entry.provider.as_str(),
                            entry.resource_kind
                        ));
                    }
                }
                Err(error) => notices.push(format!(
                    "override {} for {} was ignored: {error}",
                    entry.id, connection.id
                )),
            }
        }
    }
    notices
}

fn rendered_override(
    entry: &TemplateOverride,
    connection: &DiscoveredConnection,
) -> Result<CommandTemplate, String> {
    if entry.id.trim().is_empty()
        || entry.label.trim().is_empty()
        || entry.command.trim().is_empty()
    {
        return Err("id, label, and command must be non-empty".to_owned());
    }
    if entry.command.contains(['\n', '\r', '\0']) {
        return Err("command must be a single printable line".to_owned());
    }
    Ok(CommandTemplate {
        id: entry.id.clone(),
        label: interpolate_override(&entry.label, connection, false)?,
        kind: entry.action,
        command: interpolate_override(&entry.command, connection, true)?,
        description: interpolate_override(&entry.description, connection, false)?,
    })
}

fn interpolate_override(
    template: &str,
    connection: &DiscoveredConnection,
    quote_values: bool,
) -> Result<String, String> {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            return Err("unclosed placeholder".to_owned());
        };
        let key = &after[..end];
        let value = match key {
            "id" => Some(connection.id.as_str()),
            "label" => Some(connection.label.as_str()),
            "provider" => Some(connection.provider.as_str()),
            "kind" => Some(connection.kind.as_str()),
            _ => connection.metadata.get(key).map(String::as_str),
        }
        .ok_or_else(|| format!("metadata placeholder {{{key}}} is unavailable"))?;
        if quote_values {
            output.push_str(&shell_arg(value));
        } else {
            output.push_str(value);
        }
        rest = &after[end + 1..];
    }
    if rest.contains('}') {
        return Err("unmatched closing brace".to_owned());
    }
    output.push_str(rest);
    Ok(output)
}

#[derive(Deserialize)]
struct KubectlConfig {
    #[serde(default, rename = "current-context")]
    current_context: String,
    #[serde(default)]
    contexts: Vec<KubectlContextEntry>,
    #[serde(default)]
    clusters: Vec<KubectlClusterEntry>,
}

#[derive(Deserialize)]
struct KubectlContextEntry {
    name: String,
    context: KubectlContext,
}

#[derive(Deserialize)]
struct KubectlContext {
    cluster: String,
    #[serde(default)]
    namespace: String,
    #[serde(default)]
    user: String,
}

#[derive(Deserialize)]
struct KubectlClusterEntry {
    name: String,
    cluster: KubectlCluster,
}

#[derive(Deserialize)]
struct KubectlCluster {
    #[serde(default)]
    server: String,
}

fn discover_kubernetes<R: CommandRunner>(runner: &R) -> (Vec<DiscoveredConnection>, SourceReport) {
    let request = CommandRequest::new("kubectl", &["config", "view", "-o", "json"]);
    let result = match runner.run(&request) {
        Ok(result) => result,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(Provider::Kubernetes, "kubectl is not installed"),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Kubernetes, format!("run kubectl: {error}")),
            );
        }
    };
    if !result.is_success() {
        return (
            Vec::new(),
            failed(Provider::Kubernetes, command_failure("kubectl", &result)),
        );
    }

    let config: KubectlConfig = match serde_json::from_str(&result.stdout) {
        Ok(config) => config,
        Err(error) => {
            return (
                Vec::new(),
                failed(
                    Provider::Kubernetes,
                    format!("parse kubectl config: {error}"),
                ),
            );
        }
    };
    let servers = config
        .clusters
        .into_iter()
        .map(|entry| (entry.name, entry.cluster.server))
        .collect::<BTreeMap<_, _>>();
    let connections = config
        .contexts
        .into_iter()
        .map(|entry| {
            let namespace = if entry.context.namespace.is_empty() {
                "default".to_owned()
            } else {
                entry.context.namespace
            };
            let mut metadata = BTreeMap::from([
                ("context".to_owned(), entry.name.clone()),
                ("cluster".to_owned(), entry.context.cluster.clone()),
                ("namespace".to_owned(), namespace.clone()),
                ("user".to_owned(), entry.context.user),
                (
                    "current".to_owned(),
                    (entry.name == config.current_context).to_string(),
                ),
            ]);
            if let Some(server) = servers.get(&entry.context.cluster)
                && !server.is_empty()
            {
                metadata.insert("server".to_owned(), server.clone());
            }
            let commands = kubernetes_commands(&entry.name, &namespace);
            DiscoveredConnection {
                id: format!("kubernetes:context:{}", entry.name),
                label: entry.name,
                provider: Provider::Kubernetes,
                kind: "context".to_owned(),
                metadata,
                commands,
            }
        })
        .collect::<Vec<_>>();
    let source = loaded(
        Provider::Kubernetes,
        connections.len(),
        "kubectl contexts loaded",
    );
    (connections, source)
}

fn kubernetes_commands(context: &str, namespace: &str) -> Vec<CommandTemplate> {
    let prefix = format!(
        "kubectl --context {} --namespace {}",
        shell_arg(context),
        shell_arg(namespace)
    );
    vec![
        command(
            "connect",
            "Cluster info",
            ActionKind::Connect,
            format!("{prefix} cluster-info"),
            "Show API endpoints and verify the selected context.",
        ),
        command(
            "port-forward",
            "Port-forward service",
            ActionKind::PortForward,
            format!("{prefix} port-forward service/<service> 8080:80"),
            "Forward a local port to a service.",
        ),
        command(
            "debug",
            "Debug workload",
            ActionKind::Debug,
            format!("{prefix} debug pod/<pod> -it --image=busybox:1.36"),
            "Attach an ephemeral debugging container.",
        ),
        command(
            "pods",
            "List pods",
            ActionKind::Inspect,
            format!("{prefix} get pods -o wide"),
            "List workload placement and readiness.",
        ),
        command(
            "logs",
            "Follow logs",
            ActionKind::Logs,
            format!("{prefix} logs -f pod/<pod> --all-containers"),
            "Follow all container logs for a pod.",
        ),
        command(
            "shell",
            "Open pod shell",
            ActionKind::Shell,
            format!("{prefix} exec -it pod/<pod> -- sh"),
            "Open a shell in a selected pod.",
        ),
        command(
            "events",
            "Recent events",
            ActionKind::Inspect,
            format!("{prefix} get events --sort-by=.lastTimestamp"),
            "Inspect recent namespace events.",
        ),
        command(
            "describe",
            "Describe pod",
            ActionKind::Inspect,
            format!("{prefix} describe pod/<pod>"),
            "Show detailed workload state.",
        ),
        command(
            "top",
            "Resource usage",
            ActionKind::Inspect,
            format!("{prefix} top pods"),
            "Show live pod CPU and memory usage.",
        ),
        command(
            "auth",
            "Check permissions",
            ActionKind::Debug,
            format!("{prefix} auth can-i --list"),
            "List effective RBAC permissions.",
        ),
    ]
}

fn discover_aws<R: CommandRunner>(
    runner: &R,
    mode: DiscoveryMode,
) -> (Vec<DiscoveredConnection>, SourceReport) {
    let request = CommandRequest::new("aws", &["configure", "list-profiles"]);
    let result = match runner.run(&request) {
        Ok(result) => result,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(Provider::Aws, "aws is not installed"),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Aws, format!("run aws: {error}")),
            );
        }
    };
    if !result.is_success() {
        return (
            Vec::new(),
            failed(Provider::Aws, command_failure("aws", &result)),
        );
    }
    let profiles = result
        .stdout
        .lines()
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut connections = profiles
        .iter()
        .map(|profile| DiscoveredConnection {
            id: format!("aws:profile:{profile}"),
            label: profile.clone(),
            provider: Provider::Aws,
            kind: "profile".to_owned(),
            metadata: BTreeMap::from([("profile".to_owned(), profile.clone())]),
            commands: aws_profile_commands(profile),
        })
        .collect::<Vec<_>>();
    let mut online_errors = Vec::new();
    if mode == DiscoveryMode::Online {
        for profile in &profiles {
            match discover_aws_instances(runner, profile) {
                Ok(instances) => connections.extend(instances),
                Err(error) => online_errors.push(format!("profile {profile}: {error}")),
            }
        }
    }
    let source = if online_errors.is_empty() {
        loaded(Provider::Aws, connections.len(), "AWS profiles loaded")
    } else {
        partial_failed(Provider::Aws, connections.len(), online_errors.join("; "))
    };
    (connections, source)
}

#[derive(Default, Deserialize)]
struct AwsDescribeInstances {
    #[serde(default, rename = "Reservations")]
    reservations: Vec<AwsReservation>,
}

#[derive(Default, Deserialize)]
struct AwsReservation {
    #[serde(default, rename = "Instances")]
    instances: Vec<AwsInstance>,
}

#[derive(Default, Deserialize)]
struct AwsInstance {
    #[serde(default, rename = "InstanceId")]
    instance_id: String,
    #[serde(default, rename = "State")]
    state: AwsInstanceState,
    #[serde(default, rename = "Placement")]
    placement: AwsPlacement,
    #[serde(default, rename = "PrivateIpAddress")]
    private_ip: String,
    #[serde(default, rename = "PublicIpAddress")]
    public_ip: String,
    #[serde(default, rename = "Tags")]
    tags: Vec<AwsTag>,
}

#[derive(Default, Deserialize)]
struct AwsInstanceState {
    #[serde(default, rename = "Name")]
    name: String,
}

#[derive(Default, Deserialize)]
struct AwsPlacement {
    #[serde(default, rename = "AvailabilityZone")]
    availability_zone: String,
}

#[derive(Default, Deserialize)]
struct AwsTag {
    #[serde(default, rename = "Key")]
    key: String,
    #[serde(default, rename = "Value")]
    value: String,
}

fn discover_aws_instances<R: CommandRunner>(
    runner: &R,
    profile: &str,
) -> Result<Vec<DiscoveredConnection>, String> {
    let request = CommandRequest {
        program: "aws".to_owned(),
        args: vec![
            "ec2".to_owned(),
            "describe-instances".to_owned(),
            "--profile".to_owned(),
            profile.to_owned(),
            "--output".to_owned(),
            "json".to_owned(),
        ],
        current_dir: None,
    };
    let result = runner
        .run(&request)
        .map_err(|error| format!("run aws EC2 discovery: {error}"))?;
    if !result.is_success() {
        return Err(command_failure("aws", &result));
    }
    let response = serde_json::from_str::<AwsDescribeInstances>(&result.stdout)
        .map_err(|error| format!("parse AWS instances: {error}"))?;
    Ok(response
        .reservations
        .into_iter()
        .flat_map(|reservation| reservation.instances)
        .filter(|instance| !instance.instance_id.is_empty())
        .map(|instance| {
            let region = aws_region_from_zone(&instance.placement.availability_zone);
            let label = instance
                .tags
                .iter()
                .find(|tag| tag.key == "Name")
                .map(|tag| tag.value.clone())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| instance.instance_id.clone());
            let mut metadata = BTreeMap::from([
                ("instance_id".to_owned(), instance.instance_id.clone()),
                ("profile".to_owned(), profile.to_owned()),
            ]);
            insert_nonempty(&mut metadata, "region", region.clone());
            insert_nonempty(
                &mut metadata,
                "availability_zone",
                instance.placement.availability_zone,
            );
            insert_nonempty(&mut metadata, "state", instance.state.name);
            insert_nonempty(&mut metadata, "private_ip", instance.private_ip);
            insert_nonempty(&mut metadata, "public_ip", instance.public_ip);
            DiscoveredConnection {
                id: format!("aws:ec2:{}:{}", profile, instance.instance_id),
                label,
                provider: Provider::Aws,
                kind: "ec2-instance".to_owned(),
                metadata,
                commands: aws_instance_commands(&instance.instance_id, profile, &region),
            }
        })
        .collect())
}

fn aws_region_from_zone(zone: &str) -> String {
    zone.strip_suffix(|character: char| character.is_ascii_alphabetic())
        .unwrap_or(zone)
        .to_owned()
}

fn aws_instance_commands(instance_id: &str, profile: &str, region: &str) -> Vec<CommandTemplate> {
    let instance_id = shell_arg(instance_id);
    let profile = shell_arg(profile);
    let region = shell_arg_or_placeholder(region, "<region>");
    let scope = format!("--profile {profile} --region {region}");
    vec![
        command(
            "ssm-session",
            "SSM session",
            ActionKind::Connect,
            format!("aws ssm start-session --target {instance_id} {scope}"),
            "Open a shell through Systems Manager Session Manager.",
        ),
        command(
            "ssm-forward",
            "SSM port-forward",
            ActionKind::PortForward,
            format!(
                "aws ssm start-session --target {instance_id} --document-name AWS-StartPortForwardingSession --parameters 'portNumber=[\"80\"],localPortNumber=[\"8080\"]' {scope}"
            ),
            "Forward a local port through Session Manager.",
        ),
        command(
            "describe",
            "Describe instance",
            ActionKind::Debug,
            format!("aws ec2 describe-instances --instance-ids {instance_id} {scope}"),
            "Inspect instance, network, and tag metadata.",
        ),
        command(
            "status",
            "Instance status",
            ActionKind::Inspect,
            format!(
                "aws ec2 describe-instance-status --include-all-instances --instance-ids {instance_id} {scope}"
            ),
            "Inspect system and instance status checks.",
        ),
        command(
            "ssm-info",
            "SSM registration",
            ActionKind::Debug,
            format!(
                "aws ssm describe-instance-information --filters Key=InstanceIds,Values={instance_id} {scope}"
            ),
            "Check Systems Manager registration and ping state.",
        ),
        command(
            "console",
            "Console output",
            ActionKind::Logs,
            format!("aws ec2 get-console-output --latest --instance-id {instance_id} {scope}"),
            "Read the latest instance console output.",
        ),
        command(
            "volumes",
            "Attached volumes",
            ActionKind::Inspect,
            format!(
                "aws ec2 describe-volumes --filters Name=attachment.instance-id,Values={instance_id} {scope}"
            ),
            "Inspect volumes attached to the instance.",
        ),
        command(
            "network-interfaces",
            "Network interfaces",
            ActionKind::Inspect,
            format!(
                "aws ec2 describe-network-interfaces --filters Name=attachment.instance-id,Values={instance_id} {scope}"
            ),
            "Inspect instance network interfaces.",
        ),
        command(
            "tags",
            "Instance tags",
            ActionKind::Inspect,
            format!(
                "aws ec2 describe-tags --filters Name=resource-id,Values={instance_id} {scope}"
            ),
            "List tags attached to the instance.",
        ),
        command(
            "cloudwatch",
            "CloudWatch metrics",
            ActionKind::Inspect,
            format!(
                "aws cloudwatch list-metrics --namespace AWS/EC2 --dimensions Name=InstanceId,Value={instance_id} {scope}"
            ),
            "List available EC2 metrics.",
        ),
    ]
}

fn aws_profile_commands(profile: &str) -> Vec<CommandTemplate> {
    let profile = shell_arg(profile);
    vec![
        command(
            "identity",
            "Caller identity",
            ActionKind::Debug,
            format!("aws sts get-caller-identity --profile {profile}"),
            "Verify the account and principal represented by this profile.",
        ),
        command(
            "managed-instances",
            "Managed instances",
            ActionKind::Inspect,
            format!("aws ssm describe-instance-information --profile {profile}"),
            "List instances available through Systems Manager.",
        ),
        command(
            "running-instances",
            "Running instances",
            ActionKind::Inspect,
            format!(
                "aws ec2 describe-instances --profile {profile} --filters Name=instance-state-name,Values=running"
            ),
            "List running EC2 instances.",
        ),
        command(
            "eks-clusters",
            "EKS clusters",
            ActionKind::Inspect,
            format!("aws eks list-clusters --profile {profile}"),
            "List Kubernetes clusters in the active region.",
        ),
        command(
            "account-aliases",
            "Account aliases",
            ActionKind::Inspect,
            format!("aws iam list-account-aliases --profile {profile}"),
            "Show human-readable account aliases.",
        ),
        command(
            "regions",
            "Enabled regions",
            ActionKind::Inspect,
            format!("aws ec2 describe-regions --profile {profile}"),
            "List regions enabled for this account.",
        ),
        command(
            "s3-buckets",
            "S3 buckets",
            ActionKind::Inspect,
            format!("aws s3api list-buckets --profile {profile}"),
            "List buckets visible to this profile.",
        ),
        command(
            "cloudformation",
            "CloudFormation stacks",
            ActionKind::Inspect,
            format!("aws cloudformation list-stacks --profile {profile}"),
            "Inspect deployed infrastructure stacks.",
        ),
        command(
            "cloudwatch",
            "Recent log groups",
            ActionKind::Logs,
            format!("aws logs describe-log-groups --profile {profile}"),
            "List CloudWatch log groups.",
        ),
        command(
            "configure",
            "Profile configuration",
            ActionKind::Debug,
            format!("aws configure list --profile {profile}"),
            "Show the selected profile's non-secret configuration sources.",
        ),
    ]
}

#[derive(Deserialize)]
struct GcloudConfiguration {
    name: String,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    properties: GcloudProperties,
}

#[derive(Default, Deserialize)]
struct GcloudProperties {
    #[serde(default)]
    core: GcloudCore,
    #[serde(default)]
    compute: GcloudCompute,
}

#[derive(Default, Deserialize)]
struct GcloudCore {
    #[serde(default)]
    account: String,
    #[serde(default)]
    project: String,
}

#[derive(Default, Deserialize)]
struct GcloudCompute {
    #[serde(default)]
    region: String,
    #[serde(default)]
    zone: String,
}

fn discover_gcloud<R: CommandRunner>(
    runner: &R,
    mode: DiscoveryMode,
) -> (Vec<DiscoveredConnection>, SourceReport) {
    let request = CommandRequest::new(
        "gcloud",
        &["config", "configurations", "list", "--format=json"],
    );
    let result = match runner.run(&request) {
        Ok(result) => result,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(Provider::Gcloud, "gcloud is not installed"),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Gcloud, format!("run gcloud: {error}")),
            );
        }
    };
    if !result.is_success() {
        return (
            Vec::new(),
            failed(Provider::Gcloud, command_failure("gcloud", &result)),
        );
    }
    let configurations: Vec<GcloudConfiguration> = match serde_json::from_str(&result.stdout) {
        Ok(configurations) => configurations,
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Gcloud, format!("parse gcloud config: {error}")),
            );
        }
    };
    let mut connections = Vec::new();
    let mut online_errors = Vec::new();
    for configuration in configurations {
        let project = configuration.properties.core.project;
        let zone = configuration.properties.compute.zone;
        let mut metadata = BTreeMap::from([
            ("configuration".to_owned(), configuration.name.clone()),
            ("active".to_owned(), configuration.is_active.to_string()),
        ]);
        insert_nonempty(
            &mut metadata,
            "account",
            configuration.properties.core.account,
        );
        insert_nonempty(&mut metadata, "project", project.clone());
        insert_nonempty(
            &mut metadata,
            "region",
            configuration.properties.compute.region,
        );
        insert_nonempty(&mut metadata, "zone", zone.clone());
        connections.push(DiscoveredConnection {
            id: format!("gcloud:configuration:{}", configuration.name),
            label: configuration.name.clone(),
            provider: Provider::Gcloud,
            kind: "configuration".to_owned(),
            metadata,
            commands: gcloud_configuration_commands(&configuration.name, &project, &zone),
        });
        if mode == DiscoveryMode::Online && !project.is_empty() {
            match discover_gcloud_instances(runner, &configuration.name, &project) {
                Ok(instances) => connections.extend(instances),
                Err(error) => {
                    online_errors.push(format!("configuration {}: {error}", configuration.name))
                }
            }
        }
    }
    let source = if online_errors.is_empty() {
        loaded(
            Provider::Gcloud,
            connections.len(),
            "gcloud configurations loaded",
        )
    } else {
        partial_failed(
            Provider::Gcloud,
            connections.len(),
            online_errors.join("; "),
        )
    };
    (connections, source)
}

#[derive(Default, Deserialize)]
struct GcloudInstance {
    #[serde(default)]
    name: String,
    #[serde(default)]
    zone: String,
    #[serde(default)]
    status: String,
    #[serde(default, rename = "networkInterfaces")]
    network_interfaces: Vec<GcloudNetworkInterface>,
}

#[derive(Default, Deserialize)]
struct GcloudNetworkInterface {
    #[serde(default, rename = "networkIP")]
    network_ip: String,
    #[serde(default, rename = "accessConfigs")]
    access_configs: Vec<GcloudAccessConfig>,
}

#[derive(Default, Deserialize)]
struct GcloudAccessConfig {
    #[serde(default, rename = "natIP")]
    nat_ip: String,
}

fn discover_gcloud_instances<R: CommandRunner>(
    runner: &R,
    configuration: &str,
    project: &str,
) -> Result<Vec<DiscoveredConnection>, String> {
    let request = CommandRequest {
        program: "gcloud".to_owned(),
        args: vec![
            "--configuration".to_owned(),
            configuration.to_owned(),
            "compute".to_owned(),
            "instances".to_owned(),
            "list".to_owned(),
            "--project".to_owned(),
            project.to_owned(),
            "--format=json".to_owned(),
        ],
        current_dir: None,
    };
    let result = runner
        .run(&request)
        .map_err(|error| format!("run gcloud instance discovery: {error}"))?;
    if !result.is_success() {
        return Err(command_failure("gcloud", &result));
    }
    let instances = serde_json::from_str::<Vec<GcloudInstance>>(&result.stdout)
        .map_err(|error| format!("parse gcloud instances: {error}"))?;
    Ok(instances
        .into_iter()
        .filter(|instance| !instance.name.is_empty())
        .map(|instance| {
            let zone = instance
                .zone
                .rsplit('/')
                .next()
                .unwrap_or(&instance.zone)
                .to_owned();
            let mut metadata = BTreeMap::from([
                ("configuration".to_owned(), configuration.to_owned()),
                ("project".to_owned(), project.to_owned()),
                ("zone".to_owned(), zone.clone()),
            ]);
            insert_nonempty(&mut metadata, "status", instance.status);
            if let Some(interface) = instance.network_interfaces.first() {
                insert_nonempty(&mut metadata, "internal_ip", interface.network_ip.clone());
                if let Some(access) = interface.access_configs.first() {
                    insert_nonempty(&mut metadata, "external_ip", access.nat_ip.clone());
                }
            }
            let mut commands = gcloud_configuration_commands(configuration, project, &zone);
            let instance_arg = shell_arg(&instance.name);
            for command in &mut commands {
                command.command = command.command.replace("<instance>", &instance_arg);
            }
            DiscoveredConnection {
                id: format!("gcloud:compute:{project}:{}", instance.name),
                label: instance.name,
                provider: Provider::Gcloud,
                kind: "compute-instance".to_owned(),
                metadata,
                commands,
            }
        })
        .collect())
}

fn gcloud_configuration_commands(
    configuration: &str,
    project: &str,
    zone: &str,
) -> Vec<CommandTemplate> {
    let configuration = shell_arg(configuration);
    let project = shell_arg_or_placeholder(project, "<project>");
    let zone = shell_arg_or_placeholder(zone, "<zone>");
    let compute =
        format!("gcloud --configuration {configuration} compute --project {project} --zone {zone}");
    vec![
        command(
            "ssh",
            "SSH to instance",
            ActionKind::Connect,
            format!("{compute} ssh <instance>"),
            "Open an SSH session through gcloud.",
        ),
        command(
            "ssh-forward",
            "SSH port-forward",
            ActionKind::PortForward,
            format!("{compute} ssh <instance> -- -N -L 8080:localhost:80"),
            "Forward a local port through the instance.",
        ),
        command(
            "describe",
            "Describe instance",
            ActionKind::Debug,
            format!("{compute} instances describe <instance>"),
            "Inspect instance configuration and status.",
        ),
        command(
            "list",
            "List instances",
            ActionKind::Inspect,
            format!(
                "gcloud --configuration {configuration} compute instances list --project {project}"
            ),
            "List compute instances in the project.",
        ),
        command(
            "serial",
            "Serial output",
            ActionKind::Logs,
            format!("{compute} instances get-serial-port-output <instance>"),
            "Read recent serial console output.",
        ),
        command(
            "troubleshoot",
            "SSH troubleshoot",
            ActionKind::Debug,
            format!("{compute} ssh <instance> --troubleshoot"),
            "Run gcloud's SSH diagnostics.",
        ),
        command(
            "scp",
            "Copy from instance",
            ActionKind::Copy,
            format!("{compute} scp <instance>:/remote/path ./"),
            "Copy a file from the selected instance.",
        ),
        command(
            "start-iap",
            "IAP SSH",
            ActionKind::Connect,
            format!("{compute} ssh <instance> --tunnel-through-iap"),
            "Connect through Identity-Aware Proxy.",
        ),
        command(
            "os-login",
            "OS Login profile",
            ActionKind::Debug,
            format!(
                "gcloud --configuration {configuration} compute os-login describe-profile --project {project}"
            ),
            "Inspect the active OS Login profile.",
        ),
        command(
            "config",
            "Resolved configuration",
            ActionKind::Inspect,
            format!("gcloud --configuration {configuration} config list"),
            "Show the effective non-secret gcloud configuration.",
        ),
    ]
}

#[derive(Deserialize)]
struct AzureSubscription {
    id: String,
    name: String,
    #[serde(default, rename = "tenantId")]
    tenant_id: String,
    #[serde(default, rename = "isDefault")]
    is_default: bool,
    #[serde(default)]
    state: String,
}

fn discover_azure<R: CommandRunner>(
    runner: &R,
    mode: DiscoveryMode,
) -> (Vec<DiscoveredConnection>, SourceReport) {
    let request = CommandRequest::new("az", &["account", "list", "--output", "json"]);
    let result = match runner.run(&request) {
        Ok(result) => result,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(Provider::Azure, "az is not installed"),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Azure, format!("run az: {error}")),
            );
        }
    };
    if !result.is_success() {
        return (
            Vec::new(),
            failed(Provider::Azure, command_failure("az", &result)),
        );
    }
    let subscriptions: Vec<AzureSubscription> = match serde_json::from_str(&result.stdout) {
        Ok(subscriptions) => subscriptions,
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Azure, format!("parse az accounts: {error}")),
            );
        }
    };
    let mut connections = Vec::new();
    let mut online_errors = Vec::new();
    for subscription in subscriptions {
        let mut metadata = BTreeMap::from([
            ("subscription_id".to_owned(), subscription.id.clone()),
            ("default".to_owned(), subscription.is_default.to_string()),
        ]);
        insert_nonempty(&mut metadata, "tenant_id", subscription.tenant_id);
        insert_nonempty(&mut metadata, "state", subscription.state);
        connections.push(DiscoveredConnection {
            id: format!("azure:subscription:{}", subscription.id),
            label: subscription.name,
            provider: Provider::Azure,
            kind: "subscription".to_owned(),
            metadata,
            commands: azure_subscription_commands(&subscription.id),
        });
        if mode == DiscoveryMode::Online {
            match discover_azure_vms(runner, &subscription.id) {
                Ok(vms) => connections.extend(vms),
                Err(error) => {
                    online_errors.push(format!("subscription {}: {error}", subscription.id))
                }
            }
        }
    }
    let source = if online_errors.is_empty() {
        loaded(
            Provider::Azure,
            connections.len(),
            "Azure subscriptions loaded",
        )
    } else {
        partial_failed(Provider::Azure, connections.len(), online_errors.join("; "))
    };
    (connections, source)
}

#[derive(Default, Deserialize)]
struct AzureVirtualMachine {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "resourceGroup")]
    resource_group: String,
    #[serde(default)]
    location: String,
    #[serde(default, rename = "powerState")]
    power_state: String,
    #[serde(default, rename = "privateIps")]
    private_ips: String,
    #[serde(default, rename = "publicIps")]
    public_ips: String,
}

fn discover_azure_vms<R: CommandRunner>(
    runner: &R,
    subscription: &str,
) -> Result<Vec<DiscoveredConnection>, String> {
    let request = CommandRequest {
        program: "az".to_owned(),
        args: vec![
            "vm".to_owned(),
            "list".to_owned(),
            "--show-details".to_owned(),
            "--subscription".to_owned(),
            subscription.to_owned(),
            "--output".to_owned(),
            "json".to_owned(),
        ],
        current_dir: None,
    };
    let result = runner
        .run(&request)
        .map_err(|error| format!("run Azure VM discovery: {error}"))?;
    if !result.is_success() {
        return Err(command_failure("az", &result));
    }
    let vms = serde_json::from_str::<Vec<AzureVirtualMachine>>(&result.stdout)
        .map_err(|error| format!("parse Azure VMs: {error}"))?;
    Ok(vms
        .into_iter()
        .filter(|vm| !vm.name.is_empty() && !vm.resource_group.is_empty())
        .map(|vm| {
            let mut metadata = BTreeMap::from([
                ("subscription_id".to_owned(), subscription.to_owned()),
                ("resource_group".to_owned(), vm.resource_group.clone()),
            ]);
            insert_nonempty(&mut metadata, "resource_id", vm.id.clone());
            insert_nonempty(&mut metadata, "location", vm.location);
            insert_nonempty(&mut metadata, "power_state", vm.power_state);
            insert_nonempty(&mut metadata, "private_ip", vm.private_ips);
            insert_nonempty(&mut metadata, "public_ip", vm.public_ips);
            DiscoveredConnection {
                id: format!("azure:vm:{subscription}:{}", vm.name),
                label: vm.name.clone(),
                provider: Provider::Azure,
                kind: "virtual-machine".to_owned(),
                metadata,
                commands: azure_vm_commands(&vm.name, &vm.resource_group, subscription, &vm.id),
            }
        })
        .collect())
}

fn azure_vm_commands(
    vm: &str,
    resource_group: &str,
    subscription: &str,
    resource_id: &str,
) -> Vec<CommandTemplate> {
    let vm = shell_arg(vm);
    let resource_group = shell_arg(resource_group);
    let subscription = shell_arg(subscription);
    let resource_id = shell_arg_or_placeholder(resource_id, "<vm-resource-id>");
    let scope =
        format!("--name {vm} --resource-group {resource_group} --subscription {subscription}");
    vec![
        command(
            "ssh",
            "SSH to VM",
            ActionKind::Connect,
            format!("az ssh vm {scope}"),
            "Open an Azure CLI SSH session to this VM.",
        ),
        command(
            "bastion-tunnel",
            "Bastion tunnel",
            ActionKind::PortForward,
            format!(
                "az network bastion tunnel --name {vm}-bastion --resource-group {resource_group} --target-resource-id {resource_id} --resource-port 22 --port 2222 --subscription {subscription} # {vm}"
            ),
            "Forward a local port through Azure Bastion.",
        ),
        command(
            "instance-view",
            "VM instance view",
            ActionKind::Debug,
            format!("az vm get-instance-view {scope}"),
            "Inspect VM power and agent status.",
        ),
        command(
            "show",
            "VM details",
            ActionKind::Inspect,
            format!("az vm show {scope}"),
            "Inspect VM configuration and identity.",
        ),
        command(
            "boot-diagnostics",
            "Boot diagnostics",
            ActionKind::Logs,
            format!("az vm boot-diagnostics get-boot-log {scope}"),
            "Read VM boot diagnostics output.",
        ),
        command(
            "run-command-list",
            "Run-command capabilities",
            ActionKind::Debug,
            format!("az vm run-command list {scope}"),
            "List diagnostic commands available for the VM.",
        ),
        command(
            "nic-list",
            "Network interfaces",
            ActionKind::Inspect,
            format!("az vm nic list {scope}"),
            "Inspect VM network interfaces.",
        ),
        command(
            "disk-list",
            "Attached disks",
            ActionKind::Inspect,
            format!("az vm show {scope} --query storageProfile"),
            "Inspect OS and data disk configuration.",
        ),
        command(
            "extensions",
            "VM extensions",
            ActionKind::Inspect,
            format!(
                "az vm extension list --vm-name {vm} --resource-group {resource_group} --subscription {subscription}"
            ),
            "List installed VM extensions.",
        ),
        command(
            "metrics",
            "Metric definitions",
            ActionKind::Inspect,
            format!(
                "az monitor metrics list-definitions --resource {resource_id} --subscription {subscription} # {vm}"
            ),
            "List metrics available for this VM.",
        ),
    ]
}

fn azure_subscription_commands(subscription: &str) -> Vec<CommandTemplate> {
    let subscription = shell_arg(subscription);
    let scope = format!("--subscription {subscription}");
    vec![
        command(
            "ssh",
            "SSH to VM",
            ActionKind::Connect,
            format!("az ssh vm --name <vm> --resource-group <resource-group> {scope}"),
            "Open an Azure CLI SSH session to a VM.",
        ),
        command(
            "bastion-tunnel",
            "Bastion tunnel",
            ActionKind::PortForward,
            format!(
                "az network bastion tunnel --name <bastion> --resource-group <resource-group> --target-resource-id <vm-resource-id> --resource-port 22 --port 2222 {scope}"
            ),
            "Forward a local port through Azure Bastion.",
        ),
        command(
            "instance-view",
            "VM instance view",
            ActionKind::Debug,
            format!(
                "az vm get-instance-view --name <vm> --resource-group <resource-group> {scope}"
            ),
            "Inspect VM power and agent status.",
        ),
        command(
            "vm-list",
            "List VMs",
            ActionKind::Inspect,
            format!("az vm list --show-details {scope} --output table"),
            "List virtual machines in the subscription.",
        ),
        command(
            "aks-list",
            "List AKS clusters",
            ActionKind::Inspect,
            format!("az aks list {scope} --output table"),
            "List managed Kubernetes clusters.",
        ),
        command(
            "aks-credentials",
            "Load AKS context",
            ActionKind::Connect,
            format!(
                "az aks get-credentials --name <cluster> --resource-group <resource-group> {scope}"
            ),
            "Merge an AKS context into kubeconfig.",
        ),
        command(
            "boot-diagnostics",
            "Boot diagnostics",
            ActionKind::Logs,
            format!(
                "az vm boot-diagnostics get-boot-log --name <vm> --resource-group <resource-group> {scope}"
            ),
            "Read VM boot diagnostics output.",
        ),
        command(
            "run-command",
            "Run diagnostic script",
            ActionKind::Debug,
            format!(
                "az vm run-command invoke --command-id RunShellScript --scripts '<diagnostic-command>' --name <vm> --resource-group <resource-group> {scope}"
            ),
            "Prepare an explicit VM diagnostic invocation.",
        ),
        command(
            "network-interfaces",
            "Network interfaces",
            ActionKind::Inspect,
            format!("az network nic list {scope} --output table"),
            "Inspect subscription network interfaces.",
        ),
        command(
            "account",
            "Subscription details",
            ActionKind::Debug,
            format!("az account show {scope}"),
            "Verify active subscription and tenant metadata.",
        ),
    ]
}

fn discover_terraform(config: &DiscoveryConfig) -> (Vec<DiscoveredConnection>, SourceReport) {
    let connections = config
        .terraform_roots
        .iter()
        .filter_map(|root| {
            if !is_terraform_root(root) {
                return None;
            }
            let workspace = fs::read_to_string(root.join(".terraform/environment"))
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "default".to_owned());
            let root_label = root
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("terraform");
            let root_text = root.display().to_string();
            Some(DiscoveredConnection {
                id: format!("terraform:workspace:{root_text}:{workspace}"),
                label: format!("{root_label} / {workspace}"),
                provider: Provider::Terraform,
                kind: "workspace".to_owned(),
                metadata: BTreeMap::from([
                    ("root".to_owned(), root_text.clone()),
                    ("workspace".to_owned(), workspace),
                ]),
                commands: terraform_commands(&root_text),
            })
        })
        .collect::<Vec<_>>();
    let source = loaded(
        Provider::Terraform,
        connections.len(),
        "configured Terraform roots inspected",
    );
    (connections, source)
}

fn is_terraform_root(root: &std::path::Path) -> bool {
    if !root.is_dir() {
        return false;
    }
    root.join(".terraform").is_dir()
        || root.join("terraform.tfstate").is_file()
        || fs::read_dir(root).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "tf")
            })
        })
}

fn terraform_commands(root: &str) -> Vec<CommandTemplate> {
    let terraform = format!("terraform -chdir={}", shell_arg(root));
    vec![
        command(
            "workspace",
            "Current workspace",
            ActionKind::Inspect,
            format!("{terraform} workspace show"),
            "Show the selected Terraform workspace.",
        ),
        command(
            "show",
            "Show state",
            ActionKind::Inspect,
            format!("{terraform} show"),
            "Inspect the current state snapshot.",
        ),
        command(
            "plan",
            "Preview changes",
            ActionKind::Inspect,
            format!("{terraform} plan"),
            "Build a read-only execution plan.",
        ),
        command(
            "state-list",
            "List state resources",
            ActionKind::Inspect,
            format!("{terraform} state list"),
            "List all resource addresses in state.",
        ),
        command(
            "output",
            "Show outputs",
            ActionKind::Inspect,
            format!("{terraform} output"),
            "Display declared workspace outputs.",
        ),
        command(
            "providers",
            "Provider graph",
            ActionKind::Inspect,
            format!("{terraform} providers"),
            "Show required and configured providers.",
        ),
        command(
            "validate",
            "Validate configuration",
            ActionKind::Inspect,
            format!("{terraform} validate"),
            "Validate configuration syntax and consistency.",
        ),
        command(
            "version",
            "Terraform version",
            ActionKind::Inspect,
            format!("{terraform} version"),
            "Show Terraform and provider version selections.",
        ),
        command(
            "graph",
            "Dependency graph",
            ActionKind::Inspect,
            format!("{terraform} graph"),
            "Emit the resource dependency graph.",
        ),
        command(
            "console",
            "Expression console",
            ActionKind::Shell,
            format!("{terraform} console"),
            "Open the Terraform expression console.",
        ),
    ]
}

fn discover_ssh(config: &DiscoveryConfig) -> (Vec<DiscoveredConnection>, SourceReport) {
    let path = config.home.join(".ssh/config");
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(Provider::Ssh, "~/.ssh/config was not found"),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Ssh, format!("read {}: {error}", path.display())),
            );
        }
    };

    let mut connections = Vec::new();
    let mut aliases = Vec::<String>::new();
    let mut options = BTreeMap::<String, String>::new();
    for raw_line in input.lines().chain(std::iter::once("Host __flush__")) {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or_default().to_ascii_lowercase();
        let value = parts.next().unwrap_or_default().trim();
        if key == "host" {
            for alias in aliases.drain(..) {
                connections.push(ssh_connection(alias, &options));
            }
            options.clear();
            aliases.extend(
                value
                    .split_whitespace()
                    .filter(|alias| {
                        !alias.contains('*') && !alias.contains('?') && !alias.starts_with('!')
                    })
                    .map(str::to_owned),
            );
            continue;
        }
        if !aliases.is_empty() && matches!(key.as_str(), "hostname" | "user" | "port" | "proxyjump")
        {
            options.entry(key).or_insert_with(|| value.to_owned());
        }
    }

    let source = loaded(Provider::Ssh, connections.len(), "SSH host aliases loaded");
    (connections, source)
}

fn ssh_connection(alias: String, options: &BTreeMap<String, String>) -> DiscoveredConnection {
    let mut metadata = BTreeMap::new();
    for (source, target) in [
        ("hostname", "hostname"),
        ("user", "user"),
        ("port", "port"),
        ("proxyjump", "proxy_jump"),
    ] {
        if let Some(value) = options.get(source) {
            metadata.insert(target.to_owned(), value.clone());
        }
    }
    DiscoveredConnection {
        id: format!("ssh:host:{alias}"),
        label: alias.clone(),
        provider: Provider::Ssh,
        kind: "host".to_owned(),
        metadata,
        commands: ssh_commands(&alias),
    }
}

fn ssh_commands(alias: &str) -> Vec<CommandTemplate> {
    let alias = shell_arg(alias);
    vec![
        command(
            "connect",
            "SSH session",
            ActionKind::Connect,
            format!("ssh {alias}"),
            "Open an SSH session using the resolved host block.",
        ),
        command(
            "local-forward",
            "Local port-forward",
            ActionKind::PortForward,
            format!("ssh -N -L 8080:localhost:80 {alias}"),
            "Forward a local port through the host.",
        ),
        command(
            "verbose",
            "Verbose handshake",
            ActionKind::Debug,
            format!("ssh -vvv {alias}"),
            "Trace SSH configuration and handshake details.",
        ),
        command(
            "resolved",
            "Resolved configuration",
            ActionKind::Inspect,
            format!("ssh -G {alias}"),
            "Print the effective SSH configuration without connecting.",
        ),
        command(
            "socks",
            "SOCKS proxy",
            ActionKind::PortForward,
            format!("ssh -N -D 1080 {alias}"),
            "Create a local SOCKS proxy.",
        ),
        command(
            "remote-forward",
            "Remote port-forward",
            ActionKind::PortForward,
            format!("ssh -N -R 8080:localhost:8080 {alias}"),
            "Forward a remote port back to this machine.",
        ),
        command(
            "copy-from",
            "Copy from host",
            ActionKind::Copy,
            format!("scp {alias}:/remote/path ./"),
            "Copy a file from the host.",
        ),
        command(
            "copy-to",
            "Copy to host",
            ActionKind::Copy,
            format!("scp ./local-file {alias}:/remote/path"),
            "Copy a local file to the host.",
        ),
        command(
            "command",
            "Run remote command",
            ActionKind::Shell,
            format!("ssh {alias} '<command>'"),
            "Prepare a single remote command invocation.",
        ),
        command(
            "keepalive",
            "Keepalive session",
            ActionKind::Connect,
            format!("ssh -o ServerAliveInterval=30 -o ServerAliveCountMax=3 {alias}"),
            "Open a session with explicit keepalive settings.",
        ),
    ]
}

fn discover_docker<R: CommandRunner>(runner: &R) -> (Vec<DiscoveredConnection>, SourceReport) {
    let request = CommandRequest::new("docker", &["context", "ls", "--format", "{{json .}}"]);
    let result = match runner.run(&request) {
        Ok(result) => result,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(Provider::Docker, "docker is not installed"),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Docker, format!("run docker: {error}")),
            );
        }
    };
    if !result.is_success() {
        return (
            Vec::new(),
            failed(Provider::Docker, command_failure("docker", &result)),
        );
    }
    let records = match json_records(&result.stdout) {
        Ok(records) => records,
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Docker, format!("parse Docker contexts: {error}")),
            );
        }
    };
    let connections = records
        .into_iter()
        .filter_map(|record| {
            let name = json_string(&record, &["Name", "name"])?;
            let mut metadata = BTreeMap::new();
            if let Some(description) = json_string(&record, &["Description", "description"]) {
                insert_nonempty(&mut metadata, "description", description);
            }
            if let Some(endpoint) = json_string(&record, &["DockerEndpoint", "dockerEndpoint"]) {
                insert_nonempty(&mut metadata, "endpoint", endpoint);
            }
            if let Some(current) = record.get("Current").or_else(|| record.get("current")) {
                metadata.insert(
                    "current".to_owned(),
                    current
                        .as_str()
                        .map_or_else(|| current.to_string(), str::to_owned),
                );
            }
            Some(DiscoveredConnection {
                id: format!("docker:context:{name}"),
                label: name.clone(),
                provider: Provider::Docker,
                kind: "context".to_owned(),
                metadata,
                commands: docker_commands(&name),
            })
        })
        .collect::<Vec<_>>();
    let source = loaded(
        Provider::Docker,
        connections.len(),
        "Docker contexts loaded",
    );
    (connections, source)
}

fn docker_commands(context: &str) -> Vec<CommandTemplate> {
    let docker = format!("docker --context {}", shell_arg(context));
    vec![
        command(
            "info",
            "Daemon info",
            ActionKind::Debug,
            format!("{docker} info"),
            "Verify connectivity and inspect daemon capabilities.",
        ),
        command(
            "containers",
            "Running containers",
            ActionKind::Inspect,
            format!("{docker} ps"),
            "List running containers on this context.",
        ),
        command(
            "inspect",
            "Inspect container",
            ActionKind::Inspect,
            format!("{docker} inspect <container>"),
            "Inspect complete container metadata.",
        ),
        command(
            "logs",
            "Follow container logs",
            ActionKind::Logs,
            format!("{docker} logs -f <container>"),
            "Follow a container's output.",
        ),
        command(
            "shell",
            "Container shell",
            ActionKind::Shell,
            format!("{docker} exec -it <container> sh"),
            "Open a shell in a running container.",
        ),
        command(
            "stats",
            "Live resource usage",
            ActionKind::Inspect,
            format!("{docker} stats"),
            "Monitor container CPU and memory usage.",
        ),
        command(
            "events",
            "Daemon events",
            ActionKind::Debug,
            format!("{docker} events"),
            "Stream daemon lifecycle events.",
        ),
        command(
            "images",
            "Images",
            ActionKind::Inspect,
            format!("{docker} images"),
            "List images stored by this daemon.",
        ),
        command(
            "networks",
            "Networks",
            ActionKind::Inspect,
            format!("{docker} network ls"),
            "List Docker networks.",
        ),
        command(
            "disk",
            "Disk usage",
            ActionKind::Inspect,
            format!("{docker} system df"),
            "Show daemon disk usage without pruning data.",
        ),
    ]
}

#[derive(Default, Deserialize)]
struct TailscaleStatus {
    #[serde(default, rename = "Peer")]
    peers: BTreeMap<String, TailscalePeer>,
}

#[derive(Deserialize)]
struct TailscalePeer {
    #[serde(default, rename = "HostName")]
    hostname: String,
    #[serde(default, rename = "DNSName")]
    dns_name: String,
    #[serde(default, rename = "TailscaleIPs")]
    ips: Vec<String>,
    #[serde(default, rename = "Online")]
    online: bool,
    #[serde(default, rename = "OS")]
    os: String,
}

fn discover_tailscale<R: CommandRunner>(runner: &R) -> (Vec<DiscoveredConnection>, SourceReport) {
    let request = CommandRequest::new("tailscale", &["status", "--json"]);
    let result = match runner.run(&request) {
        Ok(result) => result,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(Provider::Tailscale, "tailscale is not installed"),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(Provider::Tailscale, format!("run tailscale: {error}")),
            );
        }
    };
    if !result.is_success() {
        return (
            Vec::new(),
            failed(Provider::Tailscale, command_failure("tailscale", &result)),
        );
    }
    let status: TailscaleStatus = match serde_json::from_str(&result.stdout) {
        Ok(status) => status,
        Err(error) => {
            return (
                Vec::new(),
                failed(
                    Provider::Tailscale,
                    format!("parse tailscale status: {error}"),
                ),
            );
        }
    };
    let connections = status
        .peers
        .into_values()
        .filter_map(|peer| {
            let address = if peer.dns_name.is_empty() {
                peer.ips.first()?.clone()
            } else {
                peer.dns_name.trim_end_matches('.').to_owned()
            };
            let label = if peer.hostname.is_empty() {
                address.clone()
            } else {
                peer.hostname
            };
            let mut metadata = BTreeMap::from([
                ("address".to_owned(), address.clone()),
                ("online".to_owned(), peer.online.to_string()),
            ]);
            if let Some(ip) = peer.ips.first() {
                metadata.insert("ip".to_owned(), ip.clone());
            }
            insert_nonempty(&mut metadata, "os", peer.os);
            Some(DiscoveredConnection {
                id: format!("tailscale:peer:{address}"),
                label,
                provider: Provider::Tailscale,
                kind: "peer".to_owned(),
                metadata,
                commands: tailscale_commands(&address),
            })
        })
        .collect::<Vec<_>>();
    let source = loaded(
        Provider::Tailscale,
        connections.len(),
        "Tailscale peers loaded",
    );
    (connections, source)
}

fn tailscale_commands(address: &str) -> Vec<CommandTemplate> {
    let address = shell_arg(address);
    vec![
        command(
            "tailscale-ssh",
            "Tailscale SSH",
            ActionKind::Connect,
            format!("tailscale ssh {address}"),
            "Open a Tailscale-authenticated SSH session.",
        ),
        command(
            "local-forward",
            "SSH port-forward",
            ActionKind::PortForward,
            format!("ssh -N -L 8080:localhost:80 {address}"),
            "Forward a local port over the tailnet.",
        ),
        command(
            "ping",
            "Tailscale ping",
            ActionKind::Debug,
            format!("tailscale ping {address}"),
            "Trace peer-to-peer connectivity and relay use.",
        ),
        command(
            "ssh",
            "Standard SSH",
            ActionKind::Connect,
            format!("ssh {address}"),
            "Open a standard SSH session over the tailnet.",
        ),
        command(
            "socks",
            "SOCKS proxy",
            ActionKind::PortForward,
            format!("ssh -N -D 1080 {address}"),
            "Create a SOCKS proxy over the tailnet.",
        ),
        command(
            "status",
            "Tailnet status",
            ActionKind::Inspect,
            "tailscale status".to_owned(),
            "Inspect all known tailnet peers.",
        ),
        command(
            "netcheck",
            "Network check",
            ActionKind::Debug,
            "tailscale netcheck".to_owned(),
            "Inspect NAT traversal and DERP reachability.",
        ),
        command(
            "copy",
            "Taildrop file",
            ActionKind::Copy,
            format!("tailscale file cp ./local-file {address}:"),
            "Send a file with Taildrop.",
        ),
        command(
            "resolved",
            "Resolved SSH config",
            ActionKind::Inspect,
            format!("ssh -G {address}"),
            "Show the effective SSH configuration.",
        ),
        command(
            "verbose-ssh",
            "Verbose SSH",
            ActionKind::Debug,
            format!("ssh -vvv {address}"),
            "Trace the SSH handshake over Tailscale.",
        ),
    ]
}

fn discover_cloudflare(config: &DiscoveryConfig) -> (Vec<DiscoveredConnection>, SourceReport) {
    let path = config.home.join(".cloudflared/config.yml");
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return (
                Vec::new(),
                unavailable(
                    Provider::Cloudflare,
                    "~/.cloudflared/config.yml was not found",
                ),
            );
        }
        Err(error) => {
            return (
                Vec::new(),
                failed(
                    Provider::Cloudflare,
                    format!("read {}: {error}", path.display()),
                ),
            );
        }
    };
    let tunnel = input
        .lines()
        .find_map(|line| yaml_value(line, "tunnel"))
        .unwrap_or_default();
    let mut entries = Vec::<(String, String)>::new();
    let mut hostname: Option<String> = None;
    for line in input.lines() {
        if let Some(value) = yaml_value(line, "hostname") {
            hostname = Some(value);
        } else if let Some(service) = yaml_value(line, "service")
            && let Some(hostname) = hostname.take()
        {
            entries.push((hostname, service));
        }
    }
    let connections = entries
        .into_iter()
        .map(|(hostname, service)| {
            let mut metadata = BTreeMap::from([
                ("hostname".to_owned(), hostname.clone()),
                ("service".to_owned(), service.clone()),
            ]);
            insert_nonempty(&mut metadata, "tunnel", tunnel.clone());
            DiscoveredConnection {
                id: format!("cloudflare:ingress:{hostname}"),
                label: hostname.clone(),
                provider: Provider::Cloudflare,
                kind: "ingress".to_owned(),
                metadata,
                commands: cloudflare_commands(&hostname, &service, &tunnel),
            }
        })
        .collect::<Vec<_>>();
    let source = loaded(
        Provider::Cloudflare,
        connections.len(),
        "Cloudflare ingress hostnames loaded",
    );
    (connections, source)
}

fn yaml_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('-').trim();
    let value = trimmed.strip_prefix(key)?.strip_prefix(':')?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.trim_matches(['\'', '"']).to_owned())
    }
}

fn cloudflare_commands(hostname: &str, service: &str, tunnel: &str) -> Vec<CommandTemplate> {
    let hostname = shell_arg(hostname);
    let tunnel = shell_arg_or_placeholder(tunnel, "<tunnel>");
    if service.starts_with("ssh://") {
        vec![
            command(
                "access-ssh",
                "Access SSH",
                ActionKind::Connect,
                format!("cloudflared access ssh --hostname {hostname}"),
                "Open an Access-authenticated SSH stream.",
            ),
            command(
                "access-tcp",
                "Access TCP tunnel",
                ActionKind::PortForward,
                format!("cloudflared access tcp --hostname {hostname} --url localhost:2222"),
                "Expose the protected SSH service on a local port.",
            ),
            command(
                "access-debug",
                "Debug Access SSH",
                ActionKind::Debug,
                format!("cloudflared access ssh --hostname {hostname} --loglevel debug"),
                "Trace Access authentication and transport setup.",
            ),
            command(
                "proxy-command",
                "SSH ProxyCommand",
                ActionKind::Connect,
                format!(
                    "ssh -o ProxyCommand='cloudflared access ssh --hostname {hostname}' <user>@{hostname}"
                ),
                "Connect with OpenSSH through cloudflared.",
            ),
            command(
                "access-login",
                "Access login",
                ActionKind::Connect,
                format!("cloudflared access login https://{hostname}"),
                "Establish an Access browser session.",
            ),
            command(
                "tunnel-info",
                "Tunnel info",
                ActionKind::Inspect,
                format!("cloudflared tunnel info {tunnel}"),
                "Inspect connector health for the configured tunnel.",
            ),
            command(
                "tunnel-route",
                "Tunnel routes",
                ActionKind::Inspect,
                "cloudflared tunnel route ip show".to_owned(),
                "List private-network tunnel routes.",
            ),
            command(
                "tunnel-list",
                "Tunnel list",
                ActionKind::Inspect,
                "cloudflared tunnel list".to_owned(),
                "List tunnels available to the active account.",
            ),
            command(
                "curl",
                "Access request",
                ActionKind::Debug,
                format!("cloudflared access curl https://{hostname}"),
                "Send an authenticated request to the hostname.",
            ),
            command(
                "dns",
                "Resolve hostname",
                ActionKind::Debug,
                format!("getent hosts {hostname}"),
                "Verify local DNS resolution for the Access hostname.",
            ),
        ]
    } else {
        vec![
            command(
                "access-curl",
                "Access request",
                ActionKind::Connect,
                format!("cloudflared access curl https://{hostname}"),
                "Send an Access-authenticated HTTP request.",
            ),
            command(
                "access-tcp",
                "Access TCP tunnel",
                ActionKind::PortForward,
                format!("cloudflared access tcp --hostname {hostname} --url localhost:8080"),
                "Expose the protected application on a local port.",
            ),
            command(
                "curl-debug",
                "Verbose request",
                ActionKind::Debug,
                format!("curl -v https://{hostname}"),
                "Trace DNS, TLS, and HTTP behavior.",
            ),
            command(
                "access-login",
                "Access login",
                ActionKind::Connect,
                format!("cloudflared access login https://{hostname}"),
                "Establish an Access browser session.",
            ),
            command(
                "headers",
                "Response headers",
                ActionKind::Inspect,
                format!("curl -I https://{hostname}"),
                "Inspect response and Access headers.",
            ),
            command(
                "tunnel-info",
                "Tunnel info",
                ActionKind::Inspect,
                format!("cloudflared tunnel info {tunnel}"),
                "Inspect connector health for the configured tunnel.",
            ),
            command(
                "tunnel-route",
                "Tunnel routes",
                ActionKind::Inspect,
                "cloudflared tunnel route ip show".to_owned(),
                "List private-network tunnel routes.",
            ),
            command(
                "tunnel-list",
                "Tunnel list",
                ActionKind::Inspect,
                "cloudflared tunnel list".to_owned(),
                "List tunnels available to the active account.",
            ),
            command(
                "dns",
                "Resolve hostname",
                ActionKind::Debug,
                format!("getent hosts {hostname}"),
                "Verify local DNS resolution.",
            ),
            command(
                "tls",
                "Inspect certificate",
                ActionKind::Debug,
                format!("openssl s_client -connect {hostname}:443 -servername {hostname}"),
                "Inspect the public TLS certificate chain.",
            ),
        ]
    }
}

fn command(
    id: &str,
    label: &str,
    kind: ActionKind,
    command: String,
    description: &str,
) -> CommandTemplate {
    CommandTemplate {
        id: id.to_owned(),
        label: label.to_owned(),
        kind,
        command,
        description: description.to_owned(),
    }
}

fn shell_arg(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-._/:@".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn shell_arg_or_placeholder(value: &str, placeholder: &str) -> String {
    if value.is_empty() {
        placeholder.to_owned()
    } else {
        shell_arg(value)
    }
}

fn insert_nonempty(metadata: &mut BTreeMap<String, String>, key: &str, value: String) {
    if !value.is_empty() {
        metadata.insert(key.to_owned(), value);
    }
}

fn json_records(input: &str) -> serde_json::Result<Vec<serde_json::Value>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        serde_json::from_str(trimmed)
    } else {
        trimmed.lines().map(serde_json::from_str).collect()
    }
}

fn json_string(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    })
}

fn loaded(provider: Provider, connections: usize, message: &str) -> SourceReport {
    SourceReport {
        provider,
        state: SourceState::Loaded,
        connections,
        message: message.to_owned(),
    }
}

fn unavailable(provider: Provider, message: &str) -> SourceReport {
    SourceReport {
        provider,
        state: SourceState::Unavailable,
        connections: 0,
        message: message.to_owned(),
    }
}

fn failed(provider: Provider, message: String) -> SourceReport {
    SourceReport {
        provider,
        state: SourceState::Failed,
        connections: 0,
        message,
    }
}

fn partial_failed(provider: Provider, connections: usize, message: String) -> SourceReport {
    SourceReport {
        provider,
        state: SourceState::Failed,
        connections,
        message,
    }
}

fn command_failure(program: &str, result: &CommandResult) -> String {
    let detail = result.stderr.trim();
    if detail.is_empty() {
        format!("{program} exited with status {}", result.status)
    } else {
        format!("{program} exited with status {}: {detail}", result.status)
    }
}
