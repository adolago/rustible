# Rustible Web UI Architecture Design

**Document Version**: 1.0
**Feature ID**: FEAT-09
**Status**: Draft
**Created**: 2025-12-26
**Author**: System Architecture Designer

---

## Executive Summary

This document describes the architecture for Rustible's Web User Interface (Web UI), providing a browser-based management console for configuration management operations. The Web UI will enable users to browse inventory, edit playbooks with syntax highlighting, execute jobs with live output streaming, and manage credentials and settings.

---

## Table of Contents

1. [Design Principles](#1-design-principles)
2. [High-Level Architecture](#2-high-level-architecture)
3. [Backend API Architecture](#3-backend-api-architecture)
4. [Component Architecture](#4-component-architecture)
5. [Inventory Browser](#5-inventory-browser)
6. [Playbook Editor](#6-playbook-editor)
7. [Job Execution View](#7-job-execution-view)
8. [Settings and Credential Management](#8-settings-and-credential-management)
9. [State Management](#9-state-management)
10. [Security Architecture](#10-security-architecture)
11. [Technology Decisions](#11-technology-decisions)
12. [Deployment Architecture](#12-deployment-architecture)
13. [Future Considerations](#13-future-considerations)

---

## 1. Design Principles

### 1.1 Core Principles

1. **Rust-Native Backend**: The API layer should be written in Rust, leveraging Rustible's existing execution engine and crate ecosystem (axum, tower).

2. **Progressive Enhancement**: The UI should work with JavaScript disabled for critical read operations, with enhanced features when JS is available.

3. **Real-Time Feedback**: Job execution should provide live output streaming via WebSockets or Server-Sent Events (SSE).

4. **Offline-First Editing**: Playbook editing should support local caching and conflict resolution for disconnected scenarios.

5. **Security by Default**: Authentication required for all operations, with role-based access control (RBAC) for multi-user environments.

6. **Responsive Design**: Mobile-friendly interface that adapts to different screen sizes.

7. **Accessibility**: WCAG 2.1 AA compliance for inclusive user experience.

### 1.2 Non-Functional Requirements

| Requirement | Target |
|-------------|--------|
| Initial Page Load | < 2 seconds |
| API Response Time | < 100ms (p95) |
| WebSocket Latency | < 50ms |
| Concurrent Users | 50+ simultaneous |
| Browser Support | Chrome, Firefox, Safari, Edge (latest 2 versions) |

---

## 2. High-Level Architecture

### 2.1 System Context Diagram (C4 Level 1)

```
                                    +-------------------+
                                    |     Operators     |
                                    |   (Web Browser)   |
                                    +--------+----------+
                                             |
                                             | HTTPS
                                             v
+-----------------------------------------------------------------------------------+
|                              Rustible Web UI System                                |
|                                                                                    |
|  +------------------+    +------------------+    +------------------------+       |
|  |                  |    |                  |    |                        |       |
|  |   Static Assets  |--->|   Web Frontend   |--->|   REST/WebSocket API   |       |
|  |   (CDN/nginx)    |    |   (SPA)          |    |   (Rust/axum)          |       |
|  |                  |    |                  |    |                        |       |
|  +------------------+    +------------------+    +-----------+------------+       |
|                                                              |                    |
|                                                              v                    |
|  +------------------+    +------------------+    +------------------------+       |
|  |                  |    |                  |    |                        |       |
|  |   File System    |<---|   Rustible Core  |<---|   Execution Engine     |       |
|  |   (Inventories,  |    |   Library        |    |   (Playbook Executor)  |       |
|  |    Playbooks)    |    |                  |    |                        |       |
|  +------------------+    +------------------+    +------------------------+       |
|                                                              |                    |
+-----------------------------------------------------------------------------------+
                                                               |
                                                               v
                                                  +------------------------+
                                                  |    Managed Hosts       |
                                                  |    (SSH/Docker/K8s)    |
                                                  +------------------------+
```

### 2.2 Container Diagram (C4 Level 2)

```
+-----------------------------------------------------------------------------------+
|                                Rustible Web UI                                     |
|                                                                                    |
|   +---------------------------+        +----------------------------------+       |
|   |      Web Frontend         |        |          API Server              |       |
|   |      (TypeScript/React)   |        |          (Rust/axum)             |       |
|   |                           |        |                                  |       |
|   |  +---------------------+  |  HTTP  |  +----------------------------+  |       |
|   |  | Inventory Browser   |--+------->|  | /api/v1/inventory          |  |       |
|   |  +---------------------+  |        |  +----------------------------+  |       |
|   |  +---------------------+  |        |  +----------------------------+  |       |
|   |  | Playbook Editor     |--+------->|  | /api/v1/playbooks          |  |       |
|   |  +---------------------+  |        |  +----------------------------+  |       |
|   |  +---------------------+  |  WS    |  +----------------------------+  |       |
|   |  | Job Execution View  |--+------->|  | /api/v1/jobs (+ /ws/jobs)  |  |       |
|   |  +---------------------+  |        |  +----------------------------+  |       |
|   |  +---------------------+  |        |  +----------------------------+  |       |
|   |  | Settings Manager    |--+------->|  | /api/v1/settings           |  |       |
|   |  +---------------------+  |        |  +----------------------------+  |       |
|   +---------------------------+        +----------------------------------+       |
|                                                      |                            |
|                                                      v                            |
|   +---------------------------+        +----------------------------------+       |
|   |      Session Store        |        |       Rustible Core              |       |
|   |      (SQLite/Redis)       |<-------|       (Library Crate)            |       |
|   +---------------------------+        +----------------------------------+       |
|                                                      |                            |
+-----------------------------------------------------------------------------------+
                                                       |
                                          +------------+------------+
                                          |            |            |
                                          v            v            v
                                      +-------+   +-------+   +---------+
                                      | Files |   |  SSH  |   | Docker  |
                                      +-------+   +-------+   +---------+
```

---

## 3. Backend API Architecture

### 3.1 API Server Design

The backend API will be implemented as a separate binary (`rustible-server`) that embeds the `rustible` library crate:

```rust
// Proposed: src/bin/server.rs or crates/rustible-server/

use axum::{
    Router,
    routing::{get, post, put, delete},
    extract::{State, WebSocketUpgrade},
};
use tower_http::cors::CorsLayer;

pub struct AppState {
    pub inventory_manager: InventoryManager,
    pub playbook_executor: PlaybookExecutor,
    pub job_manager: JobManager,
    pub credential_store: CredentialStore,
    pub session_store: SessionStore,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Inventory endpoints
        .route("/api/v1/inventory", get(list_inventory))
        .route("/api/v1/inventory/:id", get(get_inventory))
        .route("/api/v1/inventory/:id/hosts", get(list_hosts))
        .route("/api/v1/inventory/:id/groups", get(list_groups))

        // Playbook endpoints
        .route("/api/v1/playbooks", get(list_playbooks).post(create_playbook))
        .route("/api/v1/playbooks/:id", get(get_playbook).put(update_playbook).delete(delete_playbook))
        .route("/api/v1/playbooks/:id/validate", post(validate_playbook))

        // Job endpoints
        .route("/api/v1/jobs", get(list_jobs).post(create_job))
        .route("/api/v1/jobs/:id", get(get_job).delete(cancel_job))
        .route("/api/v1/jobs/:id/output", get(get_job_output))
        .route("/ws/jobs/:id", get(job_websocket_handler))

        // Settings endpoints
        .route("/api/v1/settings", get(get_settings).put(update_settings))
        .route("/api/v1/credentials", get(list_credentials).post(create_credential))
        .route("/api/v1/credentials/:id", get(get_credential).put(update_credential).delete(delete_credential))

        // Auth endpoints
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/refresh", post(refresh_token))

        .with_state(state)
        .layer(CorsLayer::permissive())
}
```

### 3.2 API Resource Models

```rust
// Inventory API Models
#[derive(Serialize, Deserialize)]
pub struct InventoryListResponse {
    pub inventories: Vec<InventorySummary>,
    pub total: usize,
}

#[derive(Serialize, Deserialize)]
pub struct InventorySummary {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub host_count: usize,
    pub group_count: usize,
    pub last_modified: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct HostDetail {
    pub name: String,
    pub groups: Vec<String>,
    pub variables: serde_json::Value,
    pub connection_type: String,
    pub status: HostStatus,
}

// Playbook API Models
#[derive(Serialize, Deserialize)]
pub struct PlaybookSummary {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub play_count: usize,
    pub last_modified: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct PlaybookContent {
    pub id: String,
    pub content: String,
    pub syntax_valid: bool,
    pub validation_errors: Vec<ValidationError>,
}

// Job API Models
#[derive(Serialize, Deserialize)]
pub struct JobSummary {
    pub id: Uuid,
    pub playbook_id: String,
    pub status: JobStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub host_summary: HostSummary,
}

#[derive(Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

// WebSocket Message Types
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JobOutputMessage {
    TaskStart { host: String, task: String, timestamp: DateTime<Utc> },
    TaskResult { host: String, task: String, status: String, changed: bool, output: String },
    PlayStart { play: String, hosts: Vec<String> },
    PlayComplete { play: String, summary: PlaySummary },
    JobComplete { summary: JobSummary },
    Error { message: String },
}
```

### 3.3 Real-Time Communication

Job output streaming will use WebSockets with automatic fallback to Server-Sent Events (SSE):

```rust
// WebSocket handler for job output streaming
async fn job_websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_job_socket(socket, state, job_id))
}

async fn handle_job_socket(
    mut socket: WebSocket,
    state: AppState,
    job_id: Uuid,
) {
    // Subscribe to job output channel
    let mut rx = state.job_manager.subscribe(job_id).await;

    while let Some(msg) = rx.recv().await {
        let json = serde_json::to_string(&msg).unwrap();
        if socket.send(Message::Text(json)).await.is_err() {
            break;
        }
    }
}
```

---

## 4. Component Architecture

### 4.1 Frontend Component Hierarchy

```
<App>
  +-- <AuthProvider>
  |     +-- <Router>
  |           +-- <Layout>
  |                 +-- <Sidebar>
  |                 |     +-- <Navigation>
  |                 |     +-- <ProjectSelector>
  |                 |
  |                 +-- <MainContent>
  |                       +-- <Route path="/inventory">
  |                       |     +-- <InventoryBrowser>
  |                       |           +-- <InventoryTree>
  |                       |           +-- <HostDetail>
  |                       |           +-- <GroupDetail>
  |                       |
  |                       +-- <Route path="/playbooks">
  |                       |     +-- <PlaybookList>
  |                       |     +-- <PlaybookEditor>
  |                       |           +-- <CodeEditor>
  |                       |           +-- <ValidationPanel>
  |                       |           +-- <TaskOutline>
  |                       |
  |                       +-- <Route path="/jobs">
  |                       |     +-- <JobList>
  |                       |     +-- <JobExecutionView>
  |                       |           +-- <JobControls>
  |                       |           +-- <LiveOutput>
  |                       |           +-- <HostStatusGrid>
  |                       |
  |                       +-- <Route path="/settings">
  |                             +-- <SettingsManager>
  |                                   +-- <GeneralSettings>
  |                                   +-- <CredentialManager>
  |                                   +-- <UserManagement>
```

### 4.2 Component Data Flow

```
+---------------------+
|   API Layer         |
|   (REST/WebSocket)  |
+----------+----------+
           |
           v
+----------+----------+
|   State Management  |
|   (Zustand/Jotai)   |
+----------+----------+
           |
    +------+------+
    |             |
    v             v
+-------+   +---------+
| Query |   | Mutation|
| Hooks |   | Hooks   |
| (GET) |   | (POST)  |
+---+---+   +----+----+
    |            |
    v            v
+----------------------+
|     UI Components    |
+----------------------+
```

---

## 5. Inventory Browser

### 5.1 Component Design

The Inventory Browser provides a hierarchical view of hosts and groups:

```
+------------------------------------------------------------------+
|  INVENTORY BROWSER                                    [Refresh]  |
+------------------------------------------------------------------+
|  +------------------+  +-------------------------------------+   |
|  | INVENTORY TREE   |  |          HOST/GROUP DETAIL          |   |
|  |                  |  |                                     |   |
|  | v [inventory]    |  |  Host: web-server-01                |   |
|  |   v all          |  |  ---------------------------------- |   |
|  |     v webservers |  |  Connection: ssh                    |   |
|  |       > web-01   |  |  Address: 192.168.1.10             |   |
|  |       > web-02   |  |  User: ansible                      |   |
|  |     v dbservers  |  |  Status: [*] Reachable              |   |
|  |       > db-01    |  |                                     |   |
|  |     > localhost  |  |  GROUPS                             |   |
|  |                  |  |  [webservers] [production]          |   |
|  |                  |  |                                     |   |
|  |                  |  |  VARIABLES                          |   |
|  |                  |  |  +-------------------------------+  |   |
|  |                  |  |  | http_port: 80                 |  |   |
|  |                  |  |  | document_root: /var/www       |  |   |
|  |                  |  |  | nginx_version: 1.18           |  |   |
|  |                  |  |  +-------------------------------+  |   |
|  |                  |  |                                     |   |
|  |                  |  |  FACTS (Cached)                     |   |
|  |                  |  |  os_family: Debian                  |   |
|  |                  |  |  distribution: Ubuntu 22.04         |   |
|  |                  |  |  [Refresh Facts]                    |   |
|  +------------------+  +-------------------------------------+   |
+------------------------------------------------------------------+
```

### 5.2 Inventory Tree Component

```typescript
// components/inventory/InventoryTree.tsx

interface InventoryTreeProps {
  inventory: Inventory;
  selectedNode: string | null;
  onSelectNode: (nodeId: string, nodeType: 'host' | 'group') => void;
  onRefresh: () => void;
}

interface TreeNode {
  id: string;
  name: string;
  type: 'inventory' | 'group' | 'host';
  children?: TreeNode[];
  hostCount?: number;
  status?: 'reachable' | 'unreachable' | 'unknown';
}

const InventoryTree: React.FC<InventoryTreeProps> = ({
  inventory,
  selectedNode,
  onSelectNode,
  onRefresh,
}) => {
  const treeData = useMemo(() => buildTreeData(inventory), [inventory]);

  return (
    <div className="inventory-tree">
      <div className="tree-header">
        <h3>Inventory</h3>
        <Button icon={<RefreshIcon />} onClick={onRefresh} />
      </div>
      <TreeView
        data={treeData}
        selected={selectedNode}
        onSelect={onSelectNode}
        renderNode={(node) => (
          <TreeNodeItem
            node={node}
            showHostCount={node.type === 'group'}
            showStatus={node.type === 'host'}
          />
        )}
      />
    </div>
  );
};
```

### 5.3 Host Detail Panel

```typescript
// components/inventory/HostDetail.tsx

interface HostDetailProps {
  host: HostDetail;
  onTestConnection: () => void;
  onRefreshFacts: () => void;
  onEditVariables: (variables: Record<string, unknown>) => void;
}

const HostDetail: React.FC<HostDetailProps> = ({
  host,
  onTestConnection,
  onRefreshFacts,
  onEditVariables,
}) => {
  return (
    <div className="host-detail">
      <header>
        <h2>{host.name}</h2>
        <StatusBadge status={host.status} />
      </header>

      <Section title="Connection">
        <PropertyList>
          <Property label="Type" value={host.connection_type} />
          <Property label="Address" value={host.ansible_host || host.name} />
          <Property label="Port" value={host.ansible_port || 22} />
          <Property label="User" value={host.ansible_user || 'root'} />
        </PropertyList>
        <Button onClick={onTestConnection}>Test Connection</Button>
      </Section>

      <Section title="Groups">
        <TagList tags={host.groups} />
      </Section>

      <Section title="Variables">
        <VariableEditor
          variables={host.variables}
          onSave={onEditVariables}
          editable={true}
        />
      </Section>

      <Section title="Cached Facts">
        <FactsViewer facts={host.facts} />
        <Button onClick={onRefreshFacts}>Refresh Facts</Button>
      </Section>
    </div>
  );
};
```

---

## 6. Playbook Editor

### 6.1 Editor Layout Design

```
+------------------------------------------------------------------+
|  PLAYBOOK EDITOR                                                  |
+------------------------------------------------------------------+
|  [playbooks/site.yml v]  [Save] [Validate] [Run] [...]           |
+------------------------------------------------------------------+
|  +------------------+  +-------------------------------------+   |
|  | TASK OUTLINE     |  |          CODE EDITOR                 |   |
|  |                  |  |                                      |   |
|  | v Play: Config   |  |  ---                                 |   |
|  |   1. Install pkg |  |  - name: Configure webservers        |   |
|  |   2. Copy conf   |  |    hosts: webservers                 |   |
|  |   3. Template    |  |    become: true                      |   |
|  |   4. Restart svc |  |                                      |   |
|  |                  |  |    tasks:                            |   |
|  | v Play: Deploy   |  |      - name: Install nginx           |   |
|  |   1. Git clone   |  |        apt:                          |   |
|  |   2. Build app   |  |          name: nginx                 |   |
|  |   3. Deploy      |  |          state: present              |   |
|  |                  |  |                                      |   |
|  | > Handlers       |  |      - name: Copy configuration      |   |
|  |   - Restart nginx|  |        template:                     |   |
|  |                  |  |          src: nginx.conf.j2          |   |
|  +------------------+  |          dest: /etc/nginx/nginx.conf |   |
|                        |        notify: Restart nginx         |   |
|  +------------------+  |                                      |   |
|  | VALIDATION       |  |    handlers:                         |   |
|  |                  |  |      - name: Restart nginx           |   |
|  | [*] Syntax OK    |  |        service:                      |   |
|  | [!] 1 warning    |  |          name: nginx                 |   |
|  |   Line 23: ...   |  |          state: restarted            |   |
|  +------------------+  +-------------------------------------+   |
+------------------------------------------------------------------+
```

### 6.2 Code Editor Component

The playbook editor will use Monaco Editor (VS Code's editor) for syntax highlighting and intelligent features:

```typescript
// components/editor/PlaybookEditor.tsx

interface PlaybookEditorProps {
  playbook: PlaybookContent;
  onChange: (content: string) => void;
  onSave: () => void;
  onValidate: () => void;
  onRun: () => void;
}

const PlaybookEditor: React.FC<PlaybookEditorProps> = ({
  playbook,
  onChange,
  onSave,
  onValidate,
  onRun,
}) => {
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor>(null);
  const [validationResult, setValidationResult] = useState<ValidationResult | null>(null);

  // Register YAML language with Ansible-specific enhancements
  useEffect(() => {
    registerAnsibleYamlLanguage(monaco);
  }, []);

  // Real-time validation with debounce
  const debouncedValidate = useDebouncedCallback(
    async (content: string) => {
      const result = await validatePlaybook(content);
      setValidationResult(result);
      updateEditorMarkers(editorRef.current, result.errors);
    },
    500
  );

  return (
    <div className="playbook-editor">
      <EditorToolbar
        onSave={onSave}
        onValidate={onValidate}
        onRun={onRun}
        isSaving={false}
        hasChanges={playbook.hasUnsavedChanges}
      />

      <div className="editor-container">
        <TaskOutline
          content={playbook.content}
          onNavigate={(line) => editorRef.current?.revealLine(line)}
        />

        <MonacoEditor
          ref={editorRef}
          language="ansible-yaml"
          value={playbook.content}
          onChange={(value) => {
            onChange(value);
            debouncedValidate(value);
          }}
          options={{
            theme: 'rustible-dark',
            fontSize: 14,
            minimap: { enabled: true },
            lineNumbers: 'on',
            folding: true,
            wordWrap: 'on',
            formatOnPaste: true,
            automaticLayout: true,
          }}
        />

        <ValidationPanel result={validationResult} />
      </div>
    </div>
  );
};
```

### 6.3 YAML/Ansible Language Support

```typescript
// editor/ansibleLanguage.ts

export function registerAnsibleYamlLanguage(monaco: Monaco) {
  // Register custom language
  monaco.languages.register({ id: 'ansible-yaml' });

  // Token provider for syntax highlighting
  monaco.languages.setMonarchTokensProvider('ansible-yaml', {
    tokenizer: {
      root: [
        // Comments
        [/#.*$/, 'comment'],

        // Jinja2 expressions
        [/\{\{.*?\}\}/, 'jinja.expression'],
        [/\{%.*?%\}/, 'jinja.statement'],

        // YAML keys (Ansible modules highlighted differently)
        [/^\s*(name|hosts|tasks|handlers|vars|roles|become|when|register|notify|loop|with_items):/,
          'keyword.control'],

        // Module names
        [/^\s*(apt|yum|dnf|package|service|file|copy|template|command|shell|git|user|group|debug|assert):/,
          'keyword.module'],

        // Strings
        [/"([^"\\]|\\.)*$/, 'string.invalid'],
        [/'([^'\\]|\\.)*$/, 'string.invalid'],
        [/"/, 'string', '@string_double'],
        [/'/, 'string', '@string_single'],

        // Booleans
        [/\b(true|false|yes|no|True|False|Yes|No)\b/, 'keyword.boolean'],

        // Numbers
        [/\b\d+\b/, 'number'],

        // Key-value separator
        [/:/, 'delimiter'],

        // List markers
        [/^\s*-\s/, 'delimiter.list'],
      ],

      string_double: [
        [/[^\\"]+/, 'string'],
        [/"/, 'string', '@pop'],
      ],

      string_single: [
        [/[^\\']+/, 'string'],
        [/'/, 'string', '@pop'],
      ],
    },
  });

  // Autocomplete provider
  monaco.languages.registerCompletionItemProvider('ansible-yaml', {
    provideCompletionItems: (model, position) => {
      const suggestions = getAnsibleCompletions(model, position);
      return { suggestions };
    },
  });

  // Hover provider for documentation
  monaco.languages.registerHoverProvider('ansible-yaml', {
    provideHover: (model, position) => {
      return getAnsibleModuleDocumentation(model, position);
    },
  });
}

// Module documentation database
const MODULE_DOCS: Record<string, ModuleDoc> = {
  apt: {
    description: 'Manages apt packages (Debian/Ubuntu)',
    parameters: {
      name: 'Package name or list of packages',
      state: 'present, absent, latest, build-dep',
      update_cache: 'Update apt cache before install',
    },
    examples: [
      '- apt:\n    name: nginx\n    state: present',
    ],
  },
  // ... more modules
};
```

### 6.4 Task Outline Component

```typescript
// components/editor/TaskOutline.tsx

interface TaskOutlineProps {
  content: string;
  onNavigate: (line: number) => void;
}

interface OutlineItem {
  type: 'play' | 'task' | 'handler' | 'role';
  name: string;
  line: number;
  children?: OutlineItem[];
}

const TaskOutline: React.FC<TaskOutlineProps> = ({ content, onNavigate }) => {
  const outline = useMemo(() => parsePlaybookOutline(content), [content]);

  return (
    <div className="task-outline">
      <h4>Outline</h4>
      <TreeView
        data={outline}
        renderNode={(item) => (
          <OutlineItem
            item={item}
            onClick={() => onNavigate(item.line)}
          />
        )}
      />
    </div>
  );
};

function parsePlaybookOutline(content: string): OutlineItem[] {
  const lines = content.split('\n');
  const outline: OutlineItem[] = [];
  let currentPlay: OutlineItem | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const lineNum = i + 1;

    // Detect play start
    if (/^\s*- name:/.test(line) && !currentPlay) {
      const name = line.match(/name:\s*["']?(.+?)["']?\s*$/)?.[1] || 'Unnamed Play';
      currentPlay = { type: 'play', name, line: lineNum, children: [] };
      outline.push(currentPlay);
    }

    // Detect task
    if (/^\s+- name:/.test(line) && currentPlay) {
      const name = line.match(/name:\s*["']?(.+?)["']?\s*$/)?.[1] || 'Unnamed Task';
      currentPlay.children?.push({ type: 'task', name, line: lineNum });
    }

    // Detect handlers section
    if (/^\s+handlers:/.test(line)) {
      // Switch to handler parsing mode
    }
  }

  return outline;
}
```

---

## 7. Job Execution View

### 7.1 Execution View Design

```
+------------------------------------------------------------------+
|  JOB EXECUTION                                        [Stop Job]  |
+------------------------------------------------------------------+
|  Playbook: site.yml  |  Status: RUNNING  |  Duration: 00:01:23   |
+------------------------------------------------------------------+
|  +----------------------------------------------------------+    |
|  | HOST STATUS GRID                                          |    |
|  |                                                           |    |
|  |  web-01    [################    ] 80%  Task: Restart svc  |    |
|  |  web-02    [####################] 100% COMPLETE           |    |
|  |  db-01     [############        ] 60%  Task: Configure    |    |
|  |  app-01    [                    ] 0%   PENDING            |    |
|  +----------------------------------------------------------+    |
|                                                                   |
|  +----------------------------------------------------------+    |
|  | LIVE OUTPUT                        [All Hosts v] [Follow] |    |
|  +----------------------------------------------------------+    |
|  |                                                           |    |
|  | PLAY [Configure webservers] ****************************  |    |
|  |                                                           |    |
|  | TASK [Install nginx] ***********************************  |    |
|  | ok: [web-01]                                              |    |
|  | ok: [web-02]                                              |    |
|  |                                                           |    |
|  | TASK [Copy configuration] ******************************  |    |
|  | changed: [web-01]                                         |    |
|  | changed: [web-02]                                         |    |
|  |                                                           |    |
|  | TASK [Restart nginx] ***********************************  |    |
|  | [web-01] Running...                                       |    |
|  | _                                                         |    |
|  +----------------------------------------------------------+    |
|                                                                   |
|  +----------------------------------------------------------+    |
|  | SUMMARY                                                   |    |
|  |                                                           |    |
|  |  Hosts: 4  |  OK: 6  |  Changed: 4  |  Failed: 0         |    |
|  +----------------------------------------------------------+    |
+------------------------------------------------------------------+
```

### 7.2 Live Output Component

```typescript
// components/jobs/LiveOutput.tsx

interface LiveOutputProps {
  jobId: string;
  autoFollow: boolean;
  hostFilter: string | null;
}

interface OutputLine {
  timestamp: Date;
  host: string;
  type: 'task_start' | 'task_result' | 'play_start' | 'play_complete' | 'error';
  content: string;
  status?: 'ok' | 'changed' | 'failed' | 'skipped' | 'unreachable';
}

const LiveOutput: React.FC<LiveOutputProps> = ({
  jobId,
  autoFollow,
  hostFilter,
}) => {
  const outputRef = useRef<HTMLDivElement>(null);
  const [lines, setLines] = useState<OutputLine[]>([]);
  const [isConnected, setIsConnected] = useState(false);

  // WebSocket connection for live updates
  useEffect(() => {
    const ws = new WebSocket(`/ws/jobs/${jobId}`);

    ws.onopen = () => setIsConnected(true);
    ws.onclose = () => setIsConnected(false);

    ws.onmessage = (event) => {
      const message: JobOutputMessage = JSON.parse(event.data);
      const line = convertMessageToLine(message);
      setLines(prev => [...prev, line]);
    };

    return () => ws.close();
  }, [jobId]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (autoFollow && outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [lines, autoFollow]);

  const filteredLines = useMemo(() => {
    if (!hostFilter) return lines;
    return lines.filter(line => line.host === hostFilter || line.type === 'play_start');
  }, [lines, hostFilter]);

  return (
    <div className="live-output">
      <div className="output-header">
        <span className={`connection-status ${isConnected ? 'connected' : 'disconnected'}`}>
          {isConnected ? 'Live' : 'Disconnected'}
        </span>
      </div>

      <div className="output-content" ref={outputRef}>
        {filteredLines.map((line, index) => (
          <OutputLine key={index} line={line} />
        ))}
      </div>
    </div>
  );
};

const OutputLine: React.FC<{ line: OutputLine }> = ({ line }) => {
  const statusColors = {
    ok: 'text-green-500',
    changed: 'text-yellow-500',
    failed: 'text-red-500',
    skipped: 'text-cyan-500',
    unreachable: 'text-red-700',
  };

  return (
    <div className={`output-line ${line.type}`}>
      {line.type === 'task_start' && (
        <span className="task-header">
          TASK [{line.content}] {'*'.repeat(60 - line.content.length)}
        </span>
      )}
      {line.type === 'task_result' && (
        <span className={statusColors[line.status || 'ok']}>
          {line.status}: [{line.host}] {line.content}
        </span>
      )}
      {line.type === 'play_start' && (
        <span className="play-header">
          PLAY [{line.content}] {'*'.repeat(60 - line.content.length)}
        </span>
      )}
      {line.type === 'error' && (
        <span className="text-red-500">ERROR: {line.content}</span>
      )}
    </div>
  );
};
```

### 7.3 Host Status Grid Component

```typescript
// components/jobs/HostStatusGrid.tsx

interface HostStatus {
  host: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  currentTask: string | null;
  progress: number; // 0-100
  taskResults: {
    ok: number;
    changed: number;
    failed: number;
    skipped: number;
  };
}

interface HostStatusGridProps {
  hosts: HostStatus[];
  onSelectHost: (host: string) => void;
  selectedHost: string | null;
}

const HostStatusGrid: React.FC<HostStatusGridProps> = ({
  hosts,
  onSelectHost,
  selectedHost,
}) => {
  return (
    <div className="host-status-grid">
      {hosts.map((host) => (
        <div
          key={host.host}
          className={`host-row ${host.status} ${selectedHost === host.host ? 'selected' : ''}`}
          onClick={() => onSelectHost(host.host)}
        >
          <span className="host-name">{host.host}</span>

          <div className="progress-bar">
            <div
              className={`progress-fill ${host.status}`}
              style={{ width: `${host.progress}%` }}
            />
          </div>

          <span className="progress-text">{host.progress}%</span>

          <span className="current-task">
            {host.status === 'running' && host.currentTask}
            {host.status === 'completed' && 'COMPLETE'}
            {host.status === 'pending' && 'PENDING'}
            {host.status === 'failed' && 'FAILED'}
          </span>

          <div className="task-summary">
            <span className="ok">{host.taskResults.ok}</span>
            <span className="changed">{host.taskResults.changed}</span>
            <span className="failed">{host.taskResults.failed}</span>
          </div>
        </div>
      ))}
    </div>
  );
};
```

### 7.4 Job Execution API Integration

```typescript
// hooks/useJobExecution.ts

interface UseJobExecutionOptions {
  playbook: string;
  inventory: string;
  extraVars?: Record<string, unknown>;
  limit?: string;
  checkMode?: boolean;
  diffMode?: boolean;
}

interface UseJobExecutionResult {
  job: JobDetail | null;
  status: JobStatus;
  output: OutputLine[];
  hostStatuses: HostStatus[];
  startJob: () => Promise<void>;
  cancelJob: () => Promise<void>;
  isLoading: boolean;
  error: Error | null;
}

export function useJobExecution(options: UseJobExecutionOptions): UseJobExecutionResult {
  const [job, setJob] = useState<JobDetail | null>(null);
  const [status, setStatus] = useState<JobStatus>('pending');
  const [output, setOutput] = useState<OutputLine[]>([]);
  const [hostStatuses, setHostStatuses] = useState<HostStatus[]>([]);
  const [error, setError] = useState<Error | null>(null);

  const wsRef = useRef<WebSocket | null>(null);

  const startJob = useCallback(async () => {
    try {
      // Create job via REST API
      const response = await fetch('/api/v1/jobs', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          playbook: options.playbook,
          inventory: options.inventory,
          extra_vars: options.extraVars,
          limit: options.limit,
          check_mode: options.checkMode,
          diff_mode: options.diffMode,
        }),
      });

      const newJob = await response.json();
      setJob(newJob);
      setStatus('running');

      // Connect WebSocket for live output
      wsRef.current = new WebSocket(`/ws/jobs/${newJob.id}`);

      wsRef.current.onmessage = (event) => {
        const message: JobOutputMessage = JSON.parse(event.data);
        handleJobMessage(message);
      };

      wsRef.current.onclose = () => {
        // Fetch final status
        fetchJobStatus(newJob.id);
      };

    } catch (err) {
      setError(err as Error);
    }
  }, [options]);

  const cancelJob = useCallback(async () => {
    if (job) {
      await fetch(`/api/v1/jobs/${job.id}`, { method: 'DELETE' });
      setStatus('cancelled');
      wsRef.current?.close();
    }
  }, [job]);

  const handleJobMessage = (message: JobOutputMessage) => {
    switch (message.type) {
      case 'TaskStart':
        updateHostStatus(message.host, { currentTask: message.task, status: 'running' });
        break;
      case 'TaskResult':
        setOutput(prev => [...prev, convertToOutputLine(message)]);
        updateHostTaskResult(message.host, message.status);
        break;
      case 'JobComplete':
        setStatus('completed');
        break;
      case 'Error':
        setError(new Error(message.message));
        break;
    }
  };

  return { job, status, output, hostStatuses, startJob, cancelJob, isLoading: status === 'running', error };
}
```

---

## 8. Settings and Credential Management

### 8.1 Settings View Design

```
+------------------------------------------------------------------+
|  SETTINGS                                                         |
+------------------------------------------------------------------+
|  +------------------+  +-------------------------------------+    |
|  | SECTIONS         |  |          CONTENT                    |    |
|  |                  |  |                                     |    |
|  | > General        |  |  GENERAL SETTINGS                   |    |
|  |   Credentials    |  |  ---------------------------------- |    |
|  |   SSH            |  |                                     |    |
|  |   Execution      |  |  Default Forks: [5     ]            |    |
|  |   Notifications  |  |  Connection Timeout: [30  ] sec     |    |
|  |   Users          |  |  Gathering Facts: [x]               |    |
|  |   About          |  |  Host Key Checking: [x]             |    |
|  |                  |  |                                     |    |
|  |                  |  |  Default Inventory:                 |    |
|  |                  |  |  [inventory/hosts.yml       ] [...]  |    |
|  |                  |  |                                     |    |
|  |                  |  |  Vault Password File:               |    |
|  |                  |  |  [                          ] [...]  |    |
|  |                  |  |                                     |    |
|  |                  |  |  Log Level: [Info       v]          |    |
|  |                  |  |                                     |    |
|  |                  |  |  [Save Changes]                     |    |
|  +------------------+  +-------------------------------------+    |
+------------------------------------------------------------------+
```

### 8.2 Credential Manager Design

```
+------------------------------------------------------------------+
|  CREDENTIAL MANAGEMENT                                [+ Add New] |
+------------------------------------------------------------------+
|                                                                   |
|  +--------------------------------------------------------------+|
|  | NAME              | TYPE          | USED BY      | ACTIONS   ||
|  +--------------------------------------------------------------+|
|  | production-ssh    | SSH Key       | 12 hosts     | [Edit][X] ||
|  | staging-ssh       | SSH Key       | 5 hosts      | [Edit][X] ||
|  | vault-password    | Vault Pass    | -            | [Edit][X] ||
|  | aws-credentials   | Cloud Creds   | 3 playbooks  | [Edit][X] ||
|  | github-token      | Token         | 1 playbook   | [Edit][X] ||
|  +--------------------------------------------------------------+|
|                                                                   |
+------------------------------------------------------------------+

// Add/Edit Credential Modal
+------------------------------------------+
|  ADD NEW CREDENTIAL                 [X]  |
+------------------------------------------+
|                                          |
|  Name: [_________________________]       |
|                                          |
|  Type: [SSH Private Key       v]         |
|                                          |
|  --- SSH Key Options ---                 |
|                                          |
|  Private Key:                            |
|  +----------------------------------+    |
|  | -----BEGIN OPENSSH PRIVATE KEY---|    |
|  | ...                               |    |
|  | -----END OPENSSH PRIVATE KEY-----|    |
|  +----------------------------------+    |
|  [Upload File]                           |
|                                          |
|  Passphrase: [**************    ]        |
|                                          |
|  [Cancel]                   [Save]       |
+------------------------------------------+
```

### 8.3 Credential Store Architecture

```rust
// Secure credential storage with encryption at rest

use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use secrecy::{ExposeSecret, Secret};

#[derive(Debug)]
pub struct CredentialStore {
    db_path: PathBuf,
    cipher: Aes256Gcm,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CredentialType {
    SshKey {
        private_key: Secret<String>,
        passphrase: Option<Secret<String>>,
        public_key: Option<String>,
    },
    VaultPassword {
        password: Secret<String>,
    },
    UsernamePassword {
        username: String,
        password: Secret<String>,
    },
    Token {
        token: Secret<String>,
    },
    CloudCredentials {
        provider: String,
        credentials: serde_json::Value, // Encrypted JSON
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    pub name: String,
    pub credential_type: CredentialType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub used_by: Vec<String>, // References to hosts/playbooks
}

impl CredentialStore {
    pub async fn new(db_path: PathBuf, master_password: &str) -> Result<Self> {
        // Derive encryption key from master password
        let salt = Self::get_or_create_salt(&db_path)?;
        let key = Self::derive_key(master_password, &salt)?;
        let cipher = Aes256Gcm::new(&key);

        Ok(Self { db_path, cipher })
    }

    pub async fn store(&self, credential: &Credential) -> Result<Uuid> {
        let encrypted = self.encrypt_credential(credential)?;
        self.save_to_db(credential.id, &encrypted).await?;
        Ok(credential.id)
    }

    pub async fn retrieve(&self, id: Uuid) -> Result<Credential> {
        let encrypted = self.load_from_db(id).await?;
        self.decrypt_credential(&encrypted)
    }

    pub async fn list(&self) -> Result<Vec<CredentialSummary>> {
        // Return summaries without sensitive data
        self.list_from_db().await
    }

    fn encrypt_credential(&self, credential: &Credential) -> Result<Vec<u8>> {
        let json = serde_json::to_vec(credential)?;
        let nonce = Nonce::from_slice(&rand::random::<[u8; 12]>());
        let ciphertext = self.cipher.encrypt(nonce, json.as_ref())?;

        // Prepend nonce to ciphertext
        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    fn decrypt_credential(&self, encrypted: &[u8]) -> Result<Credential> {
        let (nonce_bytes, ciphertext) = encrypted.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self.cipher.decrypt(nonce, ciphertext)?;
        let credential: Credential = serde_json::from_slice(&plaintext)?;
        Ok(credential)
    }
}
```

### 8.4 Settings Component

```typescript
// components/settings/SettingsManager.tsx

interface Settings {
  defaults: {
    forks: number;
    timeout: number;
    gathering: boolean;
    host_key_checking: boolean;
    default_inventory: string;
    vault_password_file: string;
  };
  ssh: {
    pipelining: boolean;
    retries: number;
    control_path: string;
  };
  privilege_escalation: {
    become: boolean;
    become_method: string;
    become_user: string;
  };
  colors: {
    enabled: boolean;
  };
  logging: {
    log_level: string;
  };
}

const SettingsManager: React.FC = () => {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [activeSection, setActiveSection] = useState('general');
  const { data, isLoading, mutate } = useSettings();

  const handleSave = async () => {
    await fetch('/api/v1/settings', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(settings),
    });
    mutate();
  };

  return (
    <div className="settings-manager">
      <nav className="settings-nav">
        <NavItem active={activeSection === 'general'} onClick={() => setActiveSection('general')}>
          General
        </NavItem>
        <NavItem active={activeSection === 'credentials'} onClick={() => setActiveSection('credentials')}>
          Credentials
        </NavItem>
        <NavItem active={activeSection === 'ssh'} onClick={() => setActiveSection('ssh')}>
          SSH
        </NavItem>
        <NavItem active={activeSection === 'execution'} onClick={() => setActiveSection('execution')}>
          Execution
        </NavItem>
      </nav>

      <div className="settings-content">
        {activeSection === 'general' && (
          <GeneralSettings settings={settings} onChange={setSettings} />
        )}
        {activeSection === 'credentials' && (
          <CredentialManager />
        )}
        {activeSection === 'ssh' && (
          <SshSettings settings={settings?.ssh} onChange={(ssh) => setSettings({...settings, ssh})} />
        )}
      </div>

      <footer className="settings-footer">
        <Button variant="primary" onClick={handleSave}>Save Changes</Button>
      </footer>
    </div>
  );
};
```

---

## 9. State Management

### 9.1 State Architecture

The frontend will use Zustand for global state management with React Query for server state:

```typescript
// store/index.ts

import { create } from 'zustand';
import { devtools, persist } from 'zustand/middleware';

interface AppState {
  // UI State
  sidebarOpen: boolean;
  theme: 'light' | 'dark' | 'system';

  // Active contexts
  activeInventory: string | null;
  activePlaybook: string | null;
  activeJob: string | null;

  // Editor state
  unsavedChanges: Record<string, boolean>;
  editorTabs: EditorTab[];

  // Actions
  setSidebarOpen: (open: boolean) => void;
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setActiveInventory: (id: string | null) => void;
  openPlaybook: (id: string) => void;
  closePlaybook: (id: string) => void;
  markUnsavedChanges: (id: string, hasChanges: boolean) => void;
}

export const useAppStore = create<AppState>()(
  devtools(
    persist(
      (set) => ({
        // Initial state
        sidebarOpen: true,
        theme: 'system',
        activeInventory: null,
        activePlaybook: null,
        activeJob: null,
        unsavedChanges: {},
        editorTabs: [],

        // Actions
        setSidebarOpen: (open) => set({ sidebarOpen: open }),
        setTheme: (theme) => set({ theme }),
        setActiveInventory: (id) => set({ activeInventory: id }),

        openPlaybook: (id) => set((state) => ({
          activePlaybook: id,
          editorTabs: state.editorTabs.some(t => t.id === id)
            ? state.editorTabs
            : [...state.editorTabs, { id, type: 'playbook' }],
        })),

        closePlaybook: (id) => set((state) => ({
          editorTabs: state.editorTabs.filter(t => t.id !== id),
          activePlaybook: state.activePlaybook === id ? null : state.activePlaybook,
        })),

        markUnsavedChanges: (id, hasChanges) => set((state) => ({
          unsavedChanges: { ...state.unsavedChanges, [id]: hasChanges },
        })),
      }),
      {
        name: 'rustible-ui-storage',
        partialize: (state) => ({
          theme: state.theme,
          sidebarOpen: state.sidebarOpen,
        }),
      }
    )
  )
);
```

### 9.2 Server State with React Query

```typescript
// hooks/queries.ts

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';

// Inventory queries
export function useInventories() {
  return useQuery({
    queryKey: ['inventories'],
    queryFn: () => fetch('/api/v1/inventory').then(r => r.json()),
    staleTime: 30_000, // 30 seconds
  });
}

export function useInventory(id: string) {
  return useQuery({
    queryKey: ['inventory', id],
    queryFn: () => fetch(`/api/v1/inventory/${id}`).then(r => r.json()),
    enabled: !!id,
  });
}

export function useHosts(inventoryId: string) {
  return useQuery({
    queryKey: ['inventory', inventoryId, 'hosts'],
    queryFn: () => fetch(`/api/v1/inventory/${inventoryId}/hosts`).then(r => r.json()),
    enabled: !!inventoryId,
  });
}

// Playbook queries
export function usePlaybooks() {
  return useQuery({
    queryKey: ['playbooks'],
    queryFn: () => fetch('/api/v1/playbooks').then(r => r.json()),
    staleTime: 30_000,
  });
}

export function usePlaybook(id: string) {
  return useQuery({
    queryKey: ['playbook', id],
    queryFn: () => fetch(`/api/v1/playbooks/${id}`).then(r => r.json()),
    enabled: !!id,
  });
}

export function useUpdatePlaybook() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, content }: { id: string; content: string }) =>
      fetch(`/api/v1/playbooks/${id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content }),
      }),
    onSuccess: (_, { id }) => {
      queryClient.invalidateQueries({ queryKey: ['playbook', id] });
    },
  });
}

// Job queries
export function useJobs(filters?: JobFilters) {
  return useQuery({
    queryKey: ['jobs', filters],
    queryFn: () => fetch(`/api/v1/jobs?${new URLSearchParams(filters as any)}`).then(r => r.json()),
    refetchInterval: 5000, // Poll every 5 seconds for active jobs
  });
}

export function useCreateJob() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (params: CreateJobParams) =>
      fetch('/api/v1/jobs', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(params),
      }).then(r => r.json()),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['jobs'] });
    },
  });
}
```

---

## 10. Security Architecture

### 10.1 Authentication Flow

```
+--------+                               +--------+                    +--------+
|        |   1. Login Request            |        |                    |        |
| Client | ----------------------------> | API    | -----------------> |  Auth  |
|        |   (username, password)        | Server |   Verify Creds     | Store  |
|        |                               |        |                    |        |
|        | <---------------------------- |        | <----------------- |        |
|        |   2. JWT + Refresh Token      |        |   User Record      |        |
|        |                               |        |                    |        |
|        |   3. API Request              |        |                    |        |
|        | ----------------------------> |        |                    |        |
|        |   (Authorization: Bearer JWT) |        |                    |        |
|        |                               |        |                    |        |
|        | <---------------------------- |        |                    |        |
|        |   4. Response                 |        |                    |        |
|        |                               |        |                    |        |
|        |   5. Token Refresh            |        |                    |        |
|        | ----------------------------> |        |                    |        |
|        |   (Refresh Token)             |        |                    |        |
|        |                               |        |                    |        |
|        | <---------------------------- |        |                    |        |
|        |   6. New JWT                  |        |                    |        |
+--------+                               +--------+                    +--------+
```

### 10.2 Authorization Model

```rust
// Role-Based Access Control (RBAC)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    Admin,      // Full access
    Operator,   // Run jobs, view everything
    Developer,  // Edit playbooks, run check mode
    Viewer,     // Read-only access
}

#[derive(Debug, Clone)]
pub enum Permission {
    // Inventory
    InventoryRead,
    InventoryWrite,
    InventoryDelete,

    // Playbooks
    PlaybookRead,
    PlaybookWrite,
    PlaybookDelete,

    // Jobs
    JobCreate,
    JobCancel,
    JobView,

    // Settings
    SettingsRead,
    SettingsWrite,

    // Credentials
    CredentialRead,
    CredentialWrite,
    CredentialDelete,

    // Users (admin only)
    UserManage,
}

impl Role {
    pub fn permissions(&self) -> Vec<Permission> {
        match self {
            Role::Admin => vec![
                Permission::InventoryRead, Permission::InventoryWrite, Permission::InventoryDelete,
                Permission::PlaybookRead, Permission::PlaybookWrite, Permission::PlaybookDelete,
                Permission::JobCreate, Permission::JobCancel, Permission::JobView,
                Permission::SettingsRead, Permission::SettingsWrite,
                Permission::CredentialRead, Permission::CredentialWrite, Permission::CredentialDelete,
                Permission::UserManage,
            ],
            Role::Operator => vec![
                Permission::InventoryRead, Permission::InventoryWrite,
                Permission::PlaybookRead, Permission::PlaybookWrite,
                Permission::JobCreate, Permission::JobCancel, Permission::JobView,
                Permission::SettingsRead,
                Permission::CredentialRead,
            ],
            Role::Developer => vec![
                Permission::InventoryRead,
                Permission::PlaybookRead, Permission::PlaybookWrite,
                Permission::JobView, // Can view but not create real jobs
                Permission::SettingsRead,
            ],
            Role::Viewer => vec![
                Permission::InventoryRead,
                Permission::PlaybookRead,
                Permission::JobView,
                Permission::SettingsRead,
            ],
        }
    }
}
```

### 10.3 Security Headers and Middleware

```rust
// Security middleware configuration

use tower_http::{
    cors::CorsLayer,
    set_header::SetResponseHeaderLayer,
};

pub fn security_layers() -> impl tower::Layer<...> {
    tower::ServiceBuilder::new()
        // CORS configuration
        .layer(CorsLayer::new()
            .allow_origin(["https://rustible.example.com".parse().unwrap()])
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE])
            .max_age(Duration::from_secs(86400)))

        // Security headers
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff")
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY")
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-xss-protection"),
            HeaderValue::from_static("1; mode=block")
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains")
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' wss:"
            )
        ))
}
```

---

## 11. Technology Decisions

### 11.1 Technology Stack

| Layer | Technology | Rationale |
|-------|------------|-----------|
| **Backend Runtime** | Rust + Tokio | Native performance, memory safety, aligns with Rustible core |
| **Web Framework** | Axum | Modern, tower-based, excellent async support |
| **Database** | SQLite | Simple deployment, sufficient for UI metadata |
| **Session Store** | In-memory / Redis | Fast session lookup, optional Redis for multi-instance |
| **Frontend Framework** | React 18 | Component model, large ecosystem, TypeScript support |
| **State Management** | Zustand + React Query | Minimal boilerplate, excellent caching |
| **Code Editor** | Monaco Editor | VS Code quality, excellent language support |
| **Styling** | Tailwind CSS | Utility-first, consistent design system |
| **Build Tool** | Vite | Fast development, optimal production builds |
| **Testing** | Vitest + Playwright | Unit and E2E testing coverage |

### 11.2 Architecture Decision Record: WebSocket vs SSE

**Decision**: Use WebSockets with SSE fallback for real-time job output streaming.

**Context**: Job execution requires streaming live output to the browser with minimal latency.

**Options Considered**:
1. **WebSockets only**: Bidirectional, persistent connection
2. **Server-Sent Events only**: Simpler, but one-way
3. **Polling**: Simple but inefficient

**Decision Rationale**:
- WebSockets provide lowest latency and bidirectional communication (useful for job control)
- SSE provides fallback for environments where WebSockets are blocked
- Both are well-supported in modern browsers

### 11.3 Architecture Decision Record: Monaco vs CodeMirror

**Decision**: Use Monaco Editor for playbook editing.

**Context**: Need a code editor with YAML syntax highlighting, autocompletion, and error markers.

**Options Considered**:
1. **Monaco Editor**: VS Code's editor, feature-rich, larger bundle
2. **CodeMirror 6**: Lightweight, modular, requires more setup
3. **Ace Editor**: Mature, but less maintained

**Decision Rationale**:
- Monaco provides the most complete editing experience out of the box
- Built-in language service protocol support for future LSP integration
- Familiar to developers who use VS Code
- Bundle size acceptable for a full application (can be code-split)

---

## 12. Deployment Architecture

### 12.1 Single Binary Deployment

The primary deployment model embeds the frontend as static assets in the Rust binary:

```rust
// Build script to embed frontend assets
// build.rs

use std::process::Command;

fn main() {
    // Build frontend
    Command::new("npm")
        .args(["run", "build"])
        .current_dir("frontend")
        .status()
        .expect("Failed to build frontend");

    // Tell cargo to rerun if frontend changes
    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=frontend/package.json");
}

// Embed static files
// src/bin/server.rs

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
struct Assets;

async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    let path = if path.is_empty() { "index.html" } else { &path };

    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .header("content-type", mime.as_ref())
                .body(content.data.into())
                .unwrap()
        }
        None => {
            // SPA fallback - serve index.html for client-side routing
            let index = Assets::get("index.html").unwrap();
            Response::builder()
                .header("content-type", "text/html")
                .body(index.data.into())
                .unwrap()
        }
    }
}
```

### 12.2 Deployment Options

```
1. SINGLE BINARY (Recommended for most users)
   +------------------+
   | rustible-server  |  <- Single binary with embedded UI
   +------------------+

2. SEPARATE SERVICES (For development or large deployments)
   +------------------+     +------------------+
   | Frontend (nginx) | --> | API Server       |
   +------------------+     +------------------+

3. CONTAINER DEPLOYMENT
   +------------------+
   | Docker Container |
   |  - rustible-srv  |
   |  - Port 8080     |
   +------------------+

4. KUBERNETES
   +------------------+     +------------------+
   | Ingress          | --> | rustible Pod     |
   +------------------+     | - rustible-srv   |
                            | - Volume: data   |
                            +------------------+
```

### 12.3 Configuration

```toml
# rustible-server.toml

[server]
bind = "0.0.0.0:8080"
workers = 4

[database]
path = "/var/lib/rustible/rustible.db"

[auth]
jwt_secret = "${RUSTIBLE_JWT_SECRET}"
jwt_expiry = "1h"
refresh_expiry = "7d"

[session]
store = "memory"  # or "redis://localhost:6379"

[security]
cors_origins = ["https://rustible.example.com"]
```

---

## 13. Future Considerations

### 13.1 Phase 2 Features (Not in Initial Release)

1. **Multi-tenancy**: Support for multiple organizations with isolated inventories and credentials

2. **Role Collections Browser**: Integration with Ansible Galaxy for browsing and installing roles

3. **Graph Visualization**: Visual representation of playbook execution flow and host dependencies

4. **Audit Logging**: Complete audit trail of all operations for compliance

5. **Scheduled Jobs**: Cron-like scheduling for playbook execution

6. **Job Templates**: Saved job configurations for quick re-execution

7. **Notifications**: Webhook, email, and Slack notifications for job status

### 13.2 API Versioning Strategy

The API will follow semantic versioning:
- `v1` for initial release
- Breaking changes will increment major version (`v2`, `v3`)
- Deprecated endpoints will be maintained for at least one major version

### 13.3 Performance Optimization Opportunities

1. **Virtual Scrolling**: For large inventories and long job output
2. **Lazy Loading**: Code-split editor and heavy components
3. **Service Worker**: Offline support for viewing playbooks
4. **WebWorker**: Offload syntax parsing to background thread

---

## Appendix A: API Reference Summary

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/inventory` | GET | List all inventories |
| `/api/v1/inventory/:id` | GET | Get inventory details |
| `/api/v1/inventory/:id/hosts` | GET | List hosts in inventory |
| `/api/v1/inventory/:id/groups` | GET | List groups in inventory |
| `/api/v1/playbooks` | GET, POST | List/create playbooks |
| `/api/v1/playbooks/:id` | GET, PUT, DELETE | CRUD for playbook |
| `/api/v1/playbooks/:id/validate` | POST | Validate playbook syntax |
| `/api/v1/jobs` | GET, POST | List/create jobs |
| `/api/v1/jobs/:id` | GET, DELETE | Get job / cancel job |
| `/api/v1/jobs/:id/output` | GET | Get job output (REST) |
| `/ws/jobs/:id` | WebSocket | Stream job output |
| `/api/v1/settings` | GET, PUT | Get/update settings |
| `/api/v1/credentials` | GET, POST | List/create credentials |
| `/api/v1/credentials/:id` | GET, PUT, DELETE | CRUD for credential |
| `/api/v1/auth/login` | POST | User login |
| `/api/v1/auth/logout` | POST | User logout |
| `/api/v1/auth/refresh` | POST | Refresh JWT token |

---

## Appendix B: File Structure

```
rustible/
+-- Cargo.toml                    # Updated with server dependencies
+-- crates/
|   +-- rustible-server/
|       +-- Cargo.toml
|       +-- src/
|           +-- main.rs           # Server entry point
|           +-- api/
|           |   +-- mod.rs
|           |   +-- inventory.rs
|           |   +-- playbooks.rs
|           |   +-- jobs.rs
|           |   +-- settings.rs
|           |   +-- auth.rs
|           +-- websocket/
|           |   +-- mod.rs
|           |   +-- job_stream.rs
|           +-- models/
|           +-- auth/
|           +-- credentials/
|       +-- frontend/             # Embedded in release build
|           +-- package.json
|           +-- vite.config.ts
|           +-- src/
|               +-- main.tsx
|               +-- App.tsx
|               +-- components/
|               |   +-- inventory/
|               |   +-- editor/
|               |   +-- jobs/
|               |   +-- settings/
|               +-- hooks/
|               +-- store/
|               +-- api/
```

---

**Document Status**: Ready for Review
**Next Steps**:
1. Review by stakeholders
2. Create detailed implementation tickets
3. Begin Phase 1 implementation (API server skeleton)
