# SIEM Operator Mega Pitch (Updated)

Ниже полностью обновленный большой текст под текущее состояние проекта.
Формат: большой сценарий на 30+ минут, с акцентом на реальные фичи и ссылками на код.

---

## 0) Супер-короткий тезис

Я сделал не просто UI, а **операционный SOC-контур**: устойчивый запуск, recovery UX, роль/подтверждение критичных действий, аудит, triage, case lifecycle, event pivoting, observability и stack control.

---

## 1) Что изменилось после апдейта (главное)

После обновления проекта особенно усилились блоки:

- расширенный `siem-portal` API-шлюз (прокси в Prometheus/Alertmanager/Case/Correlator + dashboards + events search),
- отдельный `EventSearchService` с SQL-конструктором и entity context на ClickHouse,
- более сильная страница детекций в web (`DetectionsPage`) с командным контекстом/pivot-логикой,
- в `siem-operator` сохранена и углублена модель устойчивости (webview+native, автозапуск, fallback источники, RBAC, timeline, auto-triage, docker control).

---

## 2) Архитектура запуска: один бинарь, два режима

`siem-operator` по-прежнему держит стратегически правильную схему:
- WebView Unified Suite по умолчанию,
- native `egui` как fallback-контур,
- режимы выбираются аргументами и env-переменными.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/main.rs`
- диапазон: `main.rs#L4-L19`, `main.rs#L42-L69`

Почему это важно:
- не зависишь от одного UI runtime,
- проще дебажить и поддерживать,
- операционный инструмент остается живым в большем числе сред.

---

## 3) Устойчивый bootstrap и автозапуск portal

В `lib.rs` реализована зрелая цепочка старта:
- нормализация URL,
- health-check (`/health`) с fallback host,
- определение local/remote,
- опциональный autostart,
- ожидание готовности с timeout,
- kill child-процесса, если не взлетело.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/lib.rs`
- диапазон: `lib.rs#L24-L46`, `lib.rs#L67-L141`, `lib.rs#L280-L313`

Ключевая мысль для презентации:
"Запуск — это не попытка, а контролируемый процесс с тайм-бюджетом и понятным rollback".

---

## 4) Recovery UX (не бросает пользователя при сбое)

В webview-shell заложен сценарий восстановления:
- retry startup,
- open external browser,
- status/hints,
- hotkeys и прогресс-стадии.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/lib.rs`
- диапазон: `lib.rs#L342-L524`, `lib.rs#L527-L665`

Что говорить:
"Даже в деградации UI ведет пользователя к действию, а не к тупику".

---

## 5) OperatorApp как полноценный control-plane state

`OperatorApp` в `app/mod.rs` содержит большой operational state:
- cases/events/alerts/detections/assets/investigations,
- async receivers и loading-флаги,
- timeline и entity context,
- role + pending critical action,
- audit log,
- auto-triage, auto-refresh,
- theme/layout/dashboard presets,
- persisted state.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L72-L152`, `mod.rs#L154-L247`

---

## 6) Интеграционная устойчивость: proxy + direct

В операторе остается грамотная стратегия:
- сначала получать Alerts/Events через portal proxy,
- при проблемах идти напрямую в Alertmanager (`:9093`), configurable env.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L295-L323`

Это то, что реально повышает живучесть в production-like условиях.

---

## 7) Role-gated critical actions (RBAC-style guardrail)

Критичные действия (close/transition critical case):
- требуют Senior/Manager,
- иначе блок + запись в audit,
- есть pending confirmation flow.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L333-L342`, `mod.rs#L1304-L1347`
- дополнительное окно подтверждения: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/panels.rs` (`panels.rs#L85-L145`)

---

## 8) Audit trail и ответственность действий

Каждое значимое действие получает:
- timestamp,
- actor + role,
- action text,
- ограничение размера журнала.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L344-L354`

---

## 9) Auto-refresh без гонок

В апдейте это по-прежнему сильная инженерная точка:
- проверка активных fetch-потоков,
- синхронизация только когда шина не занята,
- обновление всех важных блоков.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L493-L547`

---

## 10) Persisted state с change-detection

Состояние сохраняется только при реальных изменениях snapshot.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs` (`mod.rs#L422-L491`)
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/state.rs`

---

## 11) Case lifecycle: полный рабочий цикл

Оператор умеет:
- patch case,
- create case,
- add timeline entry,
- link alert to case,
- promote alert/investigation to case.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L1209-L1281`, `mod.rs#L1389-L1433`

---

## 12) Auto-triage rules

Есть прагматичная автоматизация:
- critical без assignee → назначение oncall,
- high+auth → эскалация,
- аудит применения.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L1368-L1387`

---

## 13) Timeline, playbook и отчетность

В workflow есть:
- timeline refresh + post note,
- playbook runner (run/reset),
- export markdown report.

Ссылки:
- timeline panel: `mod.rs#L2282-L2365`
- report export: `mod.rs#L1435-L1478`
- файл: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`

---

## 14) Docker Stack Control в operator UI

Доступны stack actions из интерфейса:
- start/stop/restart/status.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs` (`mod.rs#L2500-L2532`)
- команды в palette: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/panels.rs` (`panels.rs#L67-L76`)

---

## 15) Observability snapshot + metrics series

Operator тянет состояние через portal proxy:
- build info,
- targets,
- alert counts,
- range metrics для overview.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- диапазон: `mod.rs#L3594-L3814` (fetch + обработка rx)

---

## 16) SIEM Portal как интеграционный API-хаб (очень сильный апдейт)

В `siem-portal/src/main.rs` расширенная маршрутизация:
- UI config,
- overview/infrastructure/operations/data-quality dashboards,
- detections overview,
- stack status,
- proxy в Prometheus/Alertmanager/Case/Correlator,
- events search + event detail + entity context.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/main.rs`
- диапазон: `main.rs#L54-L127`

---

## 17) handlers.rs: богатый proxy/data-access слой

`handlers.rs` подтверждает, что portal выступает API-шлюзом:
- `/api/v1/proxy/prometheus/query`
- `/api/v1/proxy/prometheus/query_range`
- `/api/v1/proxy/alertmanager/v2/alerts`
- `/api/v1/proxy/cases/*` (get/create/patch/timeline/link/investigate)
- `/api/v1/proxy/correlator/*`
- dashboard endpoints

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/handlers.rs`
- диапазон: `handlers.rs#L100-L289`

---

## 18) EventSearchService: реально серьезный блок

В апдейте здесь мощная аналитика на ClickHouse:
- фильтруемый search API,
- get event detail,
- entity context (recent events + metrics),
- SQL builder + sanitize/escape подход.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/event_search.rs`
- диапазон: `event_search.rs#L115-L135`, `event_search.rs#L137-L178`, `event_search.rs#L180-L284`, `event_search.rs#L286-L306`, `event_search.rs#L309-L361`

Что говорить:
"Event Search у нас уже не примитивный список, а полноценный pivot engine по сущностям".

---

## 19) DetectionsPage (web): прокачанная аналитика и command-driven UX

На фронте детекций есть:
- severity/state/query filters в URL state,
- приоритизация P1..P4 от severity,
- guidance-тексты,
- queue/load/share вычисления,
- commands для pivot в events/alerts/cases,
- clipboard и навигационные quick-actions.

Ссылки:
- `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/web/src/pages/DetectionsPage.tsx`
- диапазон: `DetectionsPage.tsx#L14-L20`, `L37-L51`, `L76-L87`, `L127-L150`, `L196-L275`

---

## 20) Что говорить про зрелость системы

Коротко:
- устойчивый запуск,
- resilience в интеграциях,
- role-gated critical flow,
- auditability,
- triage automation,
- case lifecycle,
- event pivoting,
- observability/stack operations в одном cockpit.

Расширенно:
"Система проектируется как operational platform shell, а не как визуальная надстройка".

---

## 21) Если спросят "у кого подсмотрел"

Правильная формулировка:
"Я опирался на best practices из SOC/DevOps/SRE инструментов: resilience-first UX, API gatewaying, least-privilege critical actions, audit trail, pivot-driven incident response".

Сравнивать по классу (без слова "копия"):
- OpenLens / Headlamp / Portainer (операторские UX-паттерны),
- подходы SRE к health/retry/recovery,
- SOC-паттерны triage + case escalation.

---

## 22) Честный и сильный ответ про позиционирование

Важно:
- это мощная desktop/operator shell для SIEM-lite,
- это не Kubernetes Operator с CRD/reconcile loop.

Такой ответ добавляет доверия.

---

## 23) Готовый длинный монолог (можно читать 10-15 минут)

Я строил этот компонент как рабочий центр управления инцидентами, где оператор не тратит время на ручное переключение между разрозненными интерфейсами. Архитектурно система стартует из одного бинаря, который поддерживает webview и native fallback, поэтому инструмент остается функциональным даже при проблемах отдельного UI-контрура. На этапе bootstrap используется health-check, локализация target-среды и автоматический запуск `siem-portal` с контролем времени готовности. Это дает предсказуемое поведение старта и снимает массу операционной рутины.

При деградации пользователь не получает тупиковую ошибку: recovery-экран содержит конкретные действия, retry-путь, внешнее открытие и контекстные подсказки. Это сокращает время восстановления и снижает зависимость от "человека, который знает где что перезапустить".

Внутри `OperatorApp` собран полноценный operational state: кейсы, алерты, события, детекции, ассеты, расследования, таймлайны, автообновление, аудит и ролевой контроль критичных операций. Критичные закрытия и переходы ограничены ролью Senior/Manager, а отказ фиксируется в audit trail. Это резко снижает риск опасных действий в инцидентном процессе.

Ключевой практический плюс — закрытый case lifecycle: создание, обновление, timeline, привязка alert и promote из alert/investigation в case. Это превращает инструмент из наблюдателя в систему управления.

Дополнительно есть triage automation, Docker stack control, и observability-снимки прямо в операторке, поэтому у аналитика и оператора единое поле принятия решений.

После обновления проекта особенно усилился `siem-portal`: он стал сильным API-hub слоем, через который идут dashboard-агрегации, прокси-запросы к Prometheus/Alertmanager/Case/Correlator и событийный поиск. Отдельный `EventSearchService` дает глубокий поиск по ClickHouse и контекст сущностей (с метриками и недавними событиями), а `DetectionsPage` в web-клиенте получила приоритизацию, guidance и command-пивоты в другие рабочие разделы.

Итог: это уже production-minded операционная оболочка SIEM-lite, которая объединяет надежность старта, устойчивость интеграций, управляемые действия, трассируемость решений и быструю реакцию в одном workflow.

---

## 24) Готовая "добивающая" фраза

"Я сделал не просто интерфейс, а отказоустойчивый SOC-operating layer: от health-aware старта и recovery UX до role-gated critical действий, audit trail, triage automation, event pivoting и полного case lifecycle с observability и stack control."

---

## 25) Быстрые ссылки на код (для отправки человеку)

- Operator main:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/main.rs`
- Operator runtime/bootstrap:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/lib.rs`
- Operator core app logic:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs`
- Operator panels:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/panels.rs`
- Portal routes:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/main.rs`
- Portal handlers:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/handlers.rs`
- Event search service:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/event_search.rs`
- Detections web page:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/web/src/pages/DetectionsPage.tsx`
---

## 26) Расшифровка диапазонов: кусок кода + что делает

Ниже для каждого диапазона из текста — короткий пример куска и простое объяснение.

### 26.1 `main.rs#L4-L19` и `main.rs#L42-L69`

```rust
fn portal_mode_from_args() -> bool {
    std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--web" | "--portal" | "-w"))
}

fn main() {
    // ... native_mode calc ...
    if !native_mode && (args.len() == 1 || portal_mode_from_args() || portal_mode_from_env()) {
        if let Err(e) = siem_operator::run_portal_webview() { /* ... */ }
        return;
    }
    if let Err(e) = siem_operator::run_egui_operator() { /* ... */ }
}
```

Что делает: определяет режим запуска и выбирает WebView или native fallback.

### 26.2 `lib.rs#L24-L46`, `lib.rs#L67-L141`, `lib.rs#L280-L313`

```rust
fn portal_ready(raw: &str, timeout: Duration) -> bool { /* health /health */ }
fn wait_for_portal(raw: &str, timeout: Duration) -> bool { /* polling */ }
fn ensure_portal_available(raw: &str) -> std::io::Result<Option<Child>> {
    // local/remote checks, autostart, timeout, kill on fail
}
```

Что делает: health-aware bootstrap с автозапуском `siem-portal` и тайм-бюджетом.

### 26.3 `lib.rs#L342-L524`, `lib.rs#L527-L665`

```rust
fn loading_screen_html(url: &str, status: &str) -> String { /* retry/open-external/copy url */ }
fn error_screen_html(url: &str, message: &str) -> String { /* recovery hints */ }
```

Что делает: формирует HTML-экраны запуска/ошибки с recover-действиями.

### 26.4 `mod.rs#L72-L152`, `mod.rs#L154-L247`

```rust
pub struct OperatorApp {
    cases: Vec<CaseBrief>,
    alerts: Vec<AlertItem>,
    events: Vec<EventRow>,
    role: UserRole,
    audit_log: Vec<AuditEntry>,
    auto_triage_enabled: bool,
    auto_refresh_enabled: bool,
    // ...
}
```

Что делает: описывает полный operational state приложения.

### 26.5 `mod.rs#L295-L323`

```rust
fn alertmanager_alerts_urls(&self) -> Vec<String> {
    let proxy = format!("{}/api/v1/proxy/alertmanager/v2/alerts", self.portal_base());
    let direct = format!("{}/api/v2/alerts", self.alertmanager_direct_base());
    vec![proxy, direct]
}
```

Что делает: задает proxy+direct fallback маршрут для алертов.

### 26.6 `mod.rs#L333-L342`, `mod.rs#L1304-L1347`, `panels.rs#L85-L145`

```rust
fn can_confirm_critical(&self) -> bool {
    matches!(self.role, UserRole::Senior | UserRole::Manager)
}
// close/transition critical -> deny or pending confirmation
```

Что делает: ограничивает критичные операции по роли и включает подтверждение.

### 26.7 `mod.rs#L344-L354`

```rust
fn append_audit(&mut self, action: String) {
    self.audit_log.insert(0, AuditEntry { /* timestamp, actor, action */ });
    self.audit_log.truncate(150);
}
```

Что делает: пишет и ограничивает журнал аудита действий.

### 26.8 `mod.rs#L493-L547`

```rust
fn has_active_fetches(&self) -> bool { /* loading flags + rx checks */ }
fn tick_auto_refresh(&mut self, ctx: &egui::Context) {
    if elapsed >= Duration::from_secs(interval) && !self.has_active_fetches() {
        self.fetch_cases();
        self.fetch_alerts();
        // ...
    }
}
```

Что делает: защищает автообновление от гонок и перегруза.

### 26.9 `mod.rs#L422-L491`

```rust
fn maybe_persist_state(&mut self) {
    let blob = serde_json::to_string(&self.to_persisted_state()).ok();
    if blob == Some(self.last_persist_blob.clone()) { return; }
    // save only on change
}
```

Что делает: сохраняет state только при реальном изменении.

### 26.10 `mod.rs#L1209-L1281`, `mod.rs#L1389-L1433`

```rust
fn patch_case_remote(&self, case_id: &str, req: &PatchCaseRequest) -> Result<(), String> { /* ... */ }
fn create_case_remote(&self, req: &CreateCaseRequest) -> Result<CreatedCaseResponse, String> { /* ... */ }
fn link_alert_remote(&self, case_id: &str, alert: &AlertItem) -> Result<(), String> { /* ... */ }
fn promote_alert_to_case(&mut self, alert: &AlertItem) -> Result<(), String> { /* ... */ }
```

Что делает: реализует core case lifecycle и promote flow.

### 26.11 `mod.rs#L1368-L1387`

```rust
fn apply_auto_triage_rules(&mut self) {
    if case.severity.eq_ignore_ascii_case("critical") && case.assignee.is_none() {
        case.assignee = Some("tier2-oncall".to_string());
    }
    // high+auth -> escalated
}
```

Что делает: автоматизирует первичный triage по правилам.

### 26.12 `mod.rs#L2282-L2365` и `mod.rs#L1435-L1478`

```rust
fn show_case_timeline_panel(&mut self, ui: &mut egui::Ui) { /* refresh/post note/playbook */ }
fn export_selected_case_markdown(&mut self) { /* writes reports/<case>.md */ }
```

Что делает: дает timeline-workflow и отчетность по кейсу.

### 26.13 `mod.rs#L2500-L2532`, `panels.rs#L67-L76`

```rust
if ui.button("Start Stack").clicked() { self.run_docker_compose_action("up"); }
if ui.button("Stop Stack").clicked() { self.run_docker_compose_action("down"); }
```

Что делает: управляет docker stack из UI и command palette.

### 26.14 `mod.rs#L3594-L3814`

```rust
fn fetch_observability_snapshot(&mut self) {
    // proxy prometheus/alertmanager queries + async rx update
}
```

Что делает: собирает observability snapshot и подает его в overview.

### 26.15 `siem-portal/src/main.rs#L54-L127`

```rust
.route("/api/v1/proxy/prometheus/query", get(handlers::proxy_prometheus_query))
.route("/api/v1/proxy/alertmanager/v2/alerts", get(handlers::proxy_alertmanager_alerts))
.route("/api/v1/events/search", get(handlers::search_events))
```

Что делает: задает API-матрицу portal как интеграционный хаб.

### 26.16 `handlers.rs#L100-L289`

```rust
pub async fn proxy_prometheus_query(...) -> Result<Response, StatusCode> { /* ... */ }
pub async fn proxy_case_timeline(...) -> Result<Response, StatusCode> { /* ... */ }
pub async fn proxy_correlator_stats(...) -> Result<Response, StatusCode> { /* ... */ }
```

Что делает: реализует прокси-доступ к внешним сервисам и case API.

### 26.17 `event_search.rs#L115-L135`, `L137-L178`, `L180-L284`, `L286-L306`, `L309-L361`

```rust
pub async fn search(&self, params: &EventSearchParams, timeout: Duration) -> Result<EventSearchResponse> { /* ... */ }
pub async fn get_event(&self, event_id: &str, timeout: Duration) -> Result<Option<EventDetail>> { /* ... */ }
pub async fn entity_context(&self, kind: &str, value: &str, timeout: Duration) -> Result<EntityContextResponse> { /* ... */ }
```

Что делает: выполняет event search/detail/entity-context через ClickHouse SQL.

### 26.18 `DetectionsPage.tsx#L14-L20`, `L37-L51`, `L76-L87`, `L127-L150`, `L196-L275`

```ts
function priorityFromSeverity(value?: string) { /* P1..P4 */ }
const filteredRows = useMemo(() => { /* severity/state/q filters */ }, [...]);
const pageCommands = useMemo<SuitePageCommand[]>(() => { /* pivots to events/alerts/cases */ }, [...]);
```

Что делает: формирует командно-ориентированный UX детекций с фильтрами и pivot.

---

## 27) Глубокий разбор с большим кодом (детально, что именно делает)

Ниже уже не "короткие куски", а расширенные фрагменты плюс объяснение почти построчно.

### 27.1 Запуск в нужном режиме (`siem-operator/src/main.rs`)

```rust
fn portal_mode_from_env() -> bool {
    std::env::var("SIEM_OPERATOR_MODE")
        .map(|v| {
            let v = v.trim();
            v.eq_ignore_ascii_case("portal")
                || v.eq_ignore_ascii_case("web")
                || v.eq_ignore_ascii_case("webview")
        })
        .unwrap_or(false)
}

fn portal_mode_from_args() -> bool {
    std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--web" | "--portal" | "-w"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let native_mode = args
        .iter()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--native" | "--egui"))
        || std::env::var("SIEM_OPERATOR_MODE")
            .map(|v| v.trim().eq_ignore_ascii_case("native"))
            .unwrap_or(false);

    if !native_mode && (args.len() == 1 || portal_mode_from_args() || portal_mode_from_env()) {
        if let Err(e) = siem_operator::run_portal_webview() {
            eprintln!("WebView / Portal: {e}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = siem_operator::run_egui_operator() {
        eprintln!("Operator (egui): {e}");
        std::process::exit(1);
    }
}
```

Детально:
- `portal_mode_from_env()` — читает `SIEM_OPERATOR_MODE` и распознает несколько алиасов режима (`portal/web/webview`), чтобы запуск был дружелюбным к разным привычкам пользователя.
- `portal_mode_from_args()` — поддержка аргументов CLI, чтобы режим можно было переключить без env.
- В `main()` сначала отрабатывается `--help`, затем вычисляется `native_mode`.
- Если native не запрошен, выбирается WebView-контур.
- При ошибке запуска любого режима процесс завершаетcя с `exit(1)`, это правильно для автоматизаций и CI-обвязок.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/main.rs` (`main.rs#L4-L69`)

### 27.2 Health-aware bootstrap (`siem-operator/src/lib.rs`)

```rust
fn portal_health_urls(raw: &str) -> Vec<String> {
    let Ok(url) = Url::parse(raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut primary = url.clone();
    primary.set_path("/health");
    primary.set_query(None);
    primary.set_fragment(None);
    out.push(primary.to_string());
    if matches!(url.host_str(), Some("localhost") | Some("::1") | Some("[::1]")) {
        let mut fallback = url;
        if fallback.set_host(Some("127.0.0.1")).is_ok() {
            fallback.set_path("/health");
            fallback.set_query(None);
            fallback.set_fragment(None);
            let candidate = fallback.to_string();
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
    out
}

fn portal_ready(raw: &str, timeout: Duration) -> bool {
    let health_urls = portal_health_urls(raw);
    if health_urls.is_empty() {
        return false;
    }
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
    else {
        return false;
    };
    health_urls.into_iter().any(|health_url| {
        client
            .get(health_url)
            .send()
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    })
}

fn wait_for_portal(raw: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if portal_ready(raw, Duration::from_secs(2)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    false
}
```

Детально:
- `portal_health_urls()` нормализует endpoint проверки здоровья и добавляет fallback на `127.0.0.1` для частых кейсов с `localhost`.
- `portal_ready()` строит client с timeout и считает сервис готовым только при успешном HTTP-статусе.
- `wait_for_portal()` реализует polling с дедлайном: это защищает от вечного зависания старта.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/lib.rs` (`lib.rs#L67-L141`)

### 27.3 Контролируемый автозапуск и отказ (`siem-operator/src/lib.rs`)

```rust
fn ensure_portal_available(raw: &str) -> std::io::Result<Option<Child>> {
    if portal_ready(raw, Duration::from_secs(2)) {
        return Ok(None);
    }

    if !portal_is_local(raw) {
        return Err(std::io::Error::other(format!(
            "portal is unavailable at {raw}. Check the remote URL, VPN/proxy/firewall, or start the suite in a browser first"
        )));
    }

    if !portal_autostart_enabled() {
        return Err(std::io::Error::other(format!(
            "portal is unavailable at {raw} and auto-start is disabled. Start `siem-portal` manually or enable SIEM_OPERATOR_AUTOSTART_PORTAL"
        )));
    }

    let repo_root = locate_repo_root().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Не удалось найти репозиторий рядом с siem-operator для автозапуска siem-portal",
        )
    })?;
    let mut child = spawn_portal_process(&repo_root, raw)?;

    if wait_for_portal(raw, Duration::from_secs(30)) {
        Ok(Some(child))
    } else {
        let _ = child.kill();
        Err(std::io::Error::other(
            "siem-operator запустил siem-portal, но портал не поднялся за 30 секунд",
        ))
    }
}
```

Детально:
- Если портал уже жив — автозапуск не делается (`Ok(None)`).
- Для удаленного URL автозапуск специально запрещен: это правильно и безопасно.
- Если автозапуск выключен флагом — возвращается четкая инструкция, что делать.
- При автозапуске ищется repo-root, стартуется child-процесс, затем идет ожидание готовности.
- Если readiness не достигнут — child убивается, чтобы не оставлять "висячий" процесс.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/lib.rs` (`lib.rs#L280-L313`)

### 27.4 Fallback для Alertmanager + RBAC + audit (`siem-operator/src/app/mod.rs`)

```rust
fn alertmanager_direct_base(&self) -> String {
    std::env::var("SIEM_OPERATOR_ALERTMANAGER_URL")
        .unwrap_or_else(|_| {
            let b = self.api_base.trim_end_matches('/');
            if let Some((scheme, rest)) = b.split_once("://") {
                let host = rest.split('/').next().unwrap_or(rest);
                let host_only = host.split(':').next().unwrap_or(host);
                format!("{scheme}://{host_only}:9093")
            } else {
                "http://127.0.0.1:9093".to_string()
            }
        })
        .trim()
        .trim_end_matches('/')
        .to_string()
}

fn alertmanager_alerts_urls(&self) -> Vec<String> {
    let proxy = format!(
        "{}/api/v1/proxy/alertmanager/v2/alerts",
        self.portal_base()
    );
    let direct = format!(
        "{}/api/v2/alerts",
        self.alertmanager_direct_base()
    );
    vec![proxy, direct]
}

fn can_confirm_critical(&self) -> bool {
    matches!(self.role, UserRole::Senior | UserRole::Manager)
}

fn append_audit(&mut self, action: String) {
    self.audit_log.insert(
        0,
        AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            actor: format!("{} ({})", self.whoami, self.role_label()),
            action,
        },
    );
    self.audit_log.truncate(150);
}
```

Детально:
- `alertmanager_direct_base()` вычисляет direct endpoint и поддерживает env override.
- `alertmanager_alerts_urls()` явно формирует два пути: portal proxy и прямой AM.
- `can_confirm_critical()` централизует ролевую проверку.
- `append_audit()` делает журнал пригодным для расследований: кто, когда, что сделал.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs` (`mod.rs#L295-L354`)

### 27.5 Auto-refresh с защитой от конкурирующих загрузок (`siem-operator/src/app/mod.rs`)

```rust
fn has_active_fetches(&self) -> bool {
    self.loading
        || self.obs_loading
        || self.events_loading
        || self.alerts_loading
        || self.detections_loading
        || self.detection_stats_loading
        || self.investigation_loading
        || self.timeline_loading
        || self.stack_status_loading
        || self.metrics_loading
        || self.assets_loading
        || self.rx.is_some()
        || self.obs_rx.is_some()
        || self.events_rx.is_some()
        || self.alerts_rx.is_some()
        || self.detections_rx.is_some()
        || self.investigation_rx.is_some()
        || self.timeline_rx.is_some()
        || self.stack_status_rx.is_some()
        || self.detection_stats_rx.is_some()
        || self.metrics_series_rx.is_some()
        || self.portal_ui_loading
        || self.portal_ui_rx.is_some()
}

fn tick_auto_refresh(&mut self, ctx: &egui::Context) {
    if !self.auto_refresh_enabled {
        return;
    }
    let interval = self.auto_refresh_interval_sec.clamp(10, 120);
    let elapsed = self.last_auto_refresh_at.elapsed();
    if elapsed >= Duration::from_secs(interval) && !self.has_active_fetches() {
        self.fetch_cases();
        self.fetch_alerts();
        self.fetch_events();
        self.fetch_detections();
        self.fetch_detection_stats();
        self.fetch_stack_status();
        self.fetch_overview_metrics_series();
        if !self.investigation_entity.trim().is_empty() {
            let entity = self.investigation_entity.clone();
            self.fetch_investigation_for_entity(&entity);
        }
        self.fetch_assets();
        self.fetch_observability_snapshot();
        self.fetch_portal_ui_config();
        self.last_auto_refresh_at = Instant::now();
        self.status = format!("Auto-refresh sync started ({}s)", interval);
    } else {
        let remaining = Duration::from_secs(interval).saturating_sub(elapsed);
        let ms = remaining.as_millis().clamp(200, 1000) as u64;
        ctx.request_repaint_after(Duration::from_millis(ms));
    }
}
```

Детально:
- `has_active_fetches()` — единая "шина занятости", учитывает и loading-флаги, и живые rx-каналы.
- `tick_auto_refresh()` запускает пачку обновлений только когда UI-шина свободна.
- Отдельный путь в `else` управляет частотой repaint, чтобы не жечь CPU.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs` (`mod.rs#L493-L547`)

### 27.6 Case lifecycle API в операторе (`siem-operator/src/app/mod.rs`)

```rust
fn patch_case_remote(&self, case_id: &str, req: &PatchCaseRequest) -> Result<(), String> {
    let url = format!("{}/api/v1/cases/{}", self.case_mgmt_base(), case_id);
    let client = Self::http_client(15)?;
    let resp = client
        .patch(&url)
        .header("X-SOC-Actor", &self.whoami)
        .json(req)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("PATCH failed: HTTP {}", resp.status()));
    }
    Ok(())
}

fn create_case_remote(&self, req: &CreateCaseRequest) -> Result<CreatedCaseResponse, String> {
    let url = format!("{}/api/v1/cases", self.case_mgmt_base());
    let client = Self::http_client(20)?;
    let resp = client
        .post(&url)
        .header("X-SOC-Actor", &self.whoami)
        .json(req)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Create case failed: HTTP {}", resp.status()));
    }
    resp.json::<CreatedCaseResponse>().map_err(|e| e.to_string())
}

fn add_timeline_remote(&self, case_id: &str, body: &str) -> Result<(), String> {
    let url = format!("{}/api/v1/cases/{}/timeline", self.case_mgmt_base(), case_id);
    let client = Self::http_client(15)?;
    let req = TimelineCreateRequest { body: body.to_string() };
    let resp = client
        .post(&url)
        .header("X-SOC-Actor", &self.whoami)
        .json(&req)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Timeline write failed: HTTP {}", resp.status()));
    }
    Ok(())
}
```

Детально:
- Все запросы идут с `X-SOC-Actor`: это связывает действие с оператором.
- На каждом шаге есть строгая проверка `status().is_success()`.
- Ошибки возвращаются с HTTP-кодом, что облегчает triage проблем API.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-operator/src/app/mod.rs` (`mod.rs#L1209-L1255`)

### 27.7 API-шлюз в `siem-portal/src/handlers.rs`

```rust
pub async fn proxy_prometheus_query(
    State(state): State<AppState>,
    Query(q): Query<PromInstantParams>,
) -> Result<Response, StatusCode> {
    let base: Url = state.cfg.prometheus.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut url = base.join("/api/v1/query").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    url.query_pairs_mut().append_pair("query", &q.query);
    if let Some(t) = &q.time {
        url.query_pairs_mut().append_pair("time", t);
    }
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}

pub async fn proxy_alertmanager_alerts(State(state): State<AppState>) -> Result<Response, StatusCode> {
    let url = format!("{}/api/v2/alerts", state.cfg.alertmanager);
    let u: Url = url.parse().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    proxy_get_json(&state.http, u, state.cfg.http_timeout).await
}

pub async fn proxy_cases(
    State(state): State<AppState>,
    Query(q): Query<CasesQuery>,
) -> Result<Response, StatusCode> {
    let mut url = join_case_management(&state, "/api/v1/cases")?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(s) = &q.status { pairs.append_pair("status", s); }
        if let Some(s) = &q.severity { pairs.append_pair("severity", s); }
        if let Some(s) = &q.assignee { pairs.append_pair("assignee", s); }
        if let Some(l) = q.limit { pairs.append_pair("limit", &l.to_string()); }
        if let Some(o) = q.offset { pairs.append_pair("offset", &o.to_string()); }
        if let Some(s) = &q.q { pairs.append_pair("q", s); }
    }
    proxy_get_json(&state.http, url, state.cfg.http_timeout).await
}
```

Детально:
- Хендлеры не хардкодят запросы "в лоб", а аккуратно собирают URL + query параметры.
- Есть единая прокси-функция `proxy_get_json`, что унифицирует таймауты и ошибки.
- `proxy_cases()` поддерживает фильтрацию и пагинацию, то есть API готов для реальной очереди кейсов.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/handlers.rs` (`handlers.rs#L100-L168`)

### 27.8 EventSearchService в `siem-portal/src/event_search.rs`

```rust
pub async fn search(
    &self,
    params: &EventSearchParams,
    timeout: std::time::Duration,
) -> Result<EventSearchResponse> {
    let filters = SearchFilters::from_params(params, &self.cfg.database)?;
    let sql = filters.build_search_sql();
    let body = self.query_json(&sql, timeout).await?;
    let rows = parse_rows(body)?
        .into_iter()
        .map(EventRow::from_json)
        .collect::<Result<Vec<_>>>()?;
    Ok(EventSearchResponse {
        meta: EventSearchMeta {
            limit: filters.limit,
            returned: rows.len(),
            filters: filters.describe(),
        },
        rows,
    })
}

pub async fn get_event(&self, event_id: &str, timeout: std::time::Duration) -> Result<Option<EventDetail>> {
    let event_id = sanitize_uuid(event_id).ok_or_else(|| anyhow!("invalid event id"))?;
    let sql = format!(
        "SELECT ... FROM {}.events WHERE event_id = toUUID('{}') LIMIT 1 FORMAT JSONEachRow",
        ident(&self.cfg.database)?,
        event_id,
    );
    let body = self.query_json(&sql, timeout).await?;
    let mut rows = parse_rows(body)?;
    let Some(row) = rows.pop() else {
        return Ok(None);
    };
    Ok(Some(EventDetail::from_json(row)?))
}
```

Детально:
- `search()` строит SQL из нормализованных фильтров, а не через unsafe raw input.
- Возвращает и данные, и `meta` (limit/returned/filters) — удобно для UI и отладки.
- `get_event()` валидирует UUID до SQL, что снижает риск невалидных/грязных запросов.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/event_search.rs` (`event_search.rs#L115-L178`)

### 27.9 Command-driven UX в `DetectionsPage.tsx`

```ts
const pageCommands = useMemo<SuitePageCommand[]>(() => {
  const commands: SuitePageCommand[] = [
    {
      id: "detections:refresh",
      title: "Refresh detection engine view",
      subtitle: "Reload firing rows, noisy rules and catalog state from the native detections API.",
      section: "Current detection view",
      keywords: "detections refresh reload rules",
      priority: 80,
      run: load,
    },
  ];

  if (severityFilter || stateFilter || q.trim()) {
    commands.push({
      id: "detections:clear-filters",
      title: "Clear detection filters",
      subtitle: "Reset severity, state and free-text filters to restore the full rule set.",
      section: "Current detection view",
      keywords: "detections clear filters reset",
      priority: 85,
      run: () => applyDetectionState({ severity: "", state: "", q: "", selected: "" }, false),
    });
  }

  if (selectedRule) {
    commands.push(
      {
        id: `detections:events:${selectedRule.id}`,
        title: `Search events for ${selectedRule.title}`,
        subtitle: "Pivot into native event search using the selected rule title.",
        section: "Selected rule",
        keywords: `${selectedRule.title} ${selectedRule.id} events`,
        priority: 100,
        run: () => navigate(`/events?q=${encodeURIComponent(selectedRule.title)}`),
      },
      {
        id: `detections:alerts:${selectedRule.id}`,
        title: "Open alert inbox for follow-up",
        subtitle: "Move from the selected rule into the alert triage queue.",
        section: "Selected rule",
        keywords: `${selectedRule.title} alerts triage`,
        priority: 85,
        run: () => navigate("/alerts"),
      },
    );
  }

  return commands;
}, [applyDetectionState, load, severityFilter, stateFilter, q, selectedRule, matchingRowsForRule, navigate]);
```

Детально:
- Команды создаются динамически, исходя из текущего контекста страницы.
- Есть действия на refresh/clear и быстрые pivot-переходы в события/алерты.
- `priority` и `keywords` позволяют использовать страницу как command center, а არა только таблицу.

Ссылка: `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/web/src/pages/DetectionsPage.tsx` (`DetectionsPage.tsx#L196-L275`)

---

## 28) Скрипты атак, поток данных и дашборды

Этот раздел описывает, как в проекте формируются события, где появляются атакующие паттерны, как они проходят обработку и как превращаются в графики и аналитические экраны.

### 28.1 Источник данных: генератор сценариев

Сценарии нормального и атакующего трафика формируются генератором в `stress/src/main.rs`.

Поддерживаются режимы:
- `normal`
- `brute-force`
- `sql-injection`
- `privilege-escalation`
- `rate-limit`
- `heavy-queries`
- `all`

Настройка идет через env:
- `SIEM_STRESS_MODE`
- `SIEM_STRESS_DURATION_SEC`
- `SIEM_STRESS_NORMAL_EPS`
- `SIEM_STRESS_ATTACK_EPS`
- `SIEM_STRESS_BATCH_SIZE`
- `SIEM_STRESS_BURST_INTERVAL_SEC`

Генератор создает структурированные события с полями `ip`, `path`, `method`, `status`, `host`, `user`, `elapsed`, `event_type`, `source_type`.

### 28.2 Ingestion и нормализация

Сырые события обрабатываются `siem-parser` (`rust-parser/src/main.rs`):
- ingest endpoint `/parse` принимает батч событий;
- pipeline выполняет парсинг и нормализацию;
- применяется PII masking;
- выполняется enrichment (GeoIP/ASN и доп. контекст);
- результат публикуется в Kafka/Redpanda.

Публичные сервисные endpoints:
- `/health`
- `/ready`
- `/metrics`

Это обеспечивает наблюдаемость ingest-цепочки и проверку состояния сервиса.

### 28.3 Детекция и агрегация сигналов

Модуль `siem-portal/src/detections.rs` собирает обзор детекций из correlator и Prometheus:
- статистика по правилам;
- список `firing_rows`;
- severity/state breakdown;
- top rules;
- расчет `critical_firing`, `firing_count`, нагрузочных метрик.

Если стандартный запрос `ALERTS` в Prometheus пустой, применяется fallback на счетчики (`increase(detection_alerts_fired_total[24h])`), чтобы детекционный экран не терял полезную картину.

### 28.4 Формирование overview-дашбордов

`siem-portal/src/overview.rs` строит аналитический слой поверх ClickHouse.

Формируемые наборы:
- KPI: total/critical/error% за окно;
- events per minute (bucketed time series);
- severity timeline;
- severity breakdown;
- source breakdown;
- top source IP;
- recent security events.

Запросы к ClickHouse выполняются параллельно через `tokio::try_join!`, благодаря чему overview получает консистентный срез данных с минимальной задержкой.

### 28.5 Поиск и расследование событий

`siem-portal/src/event_search.rs` реализует три ключевых сценария:
- `search(...)`: фильтруемый поиск по событиям;
- `get_event(...)`: детальная карточка события по `event_id`;
- `entity_context(...)`: контекст сущности (последние события + метрики за окно).

Запросы формируются SQL-конструктором по фильтрам и выполняются в ClickHouse.
Такой подход поддерживает pivot-модель: из детекции можно перейти в события и дальше в контекст сущности без потери расследовательского потока.

### 28.6 API-шлюз для UI и внешних сервисов

`siem-portal/src/handlers.rs` выступает как BFF/proxy слой:
- прокси-запросы в Prometheus (`query`, `query_range`);
- прокси-запросы в Alertmanager (`alerts`, `status`);
- прокси-операции case-management (list/create/patch/timeline/link/investigate);
- dashboard endpoints;
- search endpoints.

Это дает единый контракт для UI и изолирует фронт от деталей отдельных сервисов.

### 28.7 Как дашборды наполняются фактически

Поток данных в проекте:
1. `stress` генерирует события (включая атаки).
2. `siem-parser` нормализует и обогащает события.
3. События публикуются в очередь.
4. Детектор и аналитические сервисы читают поток и считают агрегаты.
5. `siem-portal` отдает готовые API для страниц overview/detections/events/cases.
6. UI рендерит текущую динамику по реальным данным потока.

Итог: визуализация отражает поведение конвейера и сценариев атаки, а не статически подставленные значения.

### 28.8 Ссылки на код

- Attack/stress генератор:
  - `file:///C:/Users/Admin/Проекты/siem-lite/stress/src/main.rs`
- Ingestion/parser pipeline:
  - `file:///C:/Users/Admin/Проекты/siem-lite/rust-parser/src/main.rs`
- Overview dashboards (SQL + агрегаты):
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/overview.rs`
- Detections aggregation:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/detections.rs`
- Event search + entity context:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/event_search.rs`
- Portal API handlers:
  - `file:///C:/Users/Admin/Проекты/siem-lite/siem-portal/src/handlers.rs`
- Architecture doc:
  - `file:///C:/Users/Admin/Проекты/siem-lite/docs/ARCHITECTURE.md`

---
