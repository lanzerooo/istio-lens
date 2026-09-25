# Istio Lens 🔭 & Kube-Forensics

Высокопроизводительная TUI-утилита на языке Rust для глубокого статического анализа сетевого стека Service Mesh (Istio), сквозной визуализации маршрутов трафика (от Ingress Gateway до подов приложения) и кластерной криминалистики (Kube-Forensics: анализ инцидентов, OOM-Killer, FinOps-срез нод и аудит admission-вебхуков).

---

## Ключевые возможности

* **Аудит Service Mesh:** выявление коллизий маршрутизации в `VirtualService`, дубликатов `DestinationRule` и битых маршрутов на уровне K8s-сервисов с учетом `ServiceEntry` за O(N) через инвертированный индекс.
* **Сквозная топология трафика (Hop-by-Hop Trace):** интерактивная визуализация маршрута от периметра кластера (`Gateway`) через виртуальные правила (`VirtualService`) и политики балансировки (`DestinationRule`) до реальных подов приложения с расчетом пересечения селекторов:
  `Pod Selector = Service.Spec.Selector ∪ DestinationRule.Subsets[name].Labels`
* **Causal Incident Timeline:** сведение событий пода, Kubelet и ноды в единую хронологическую шкалу времени с автоматической генерацией вердикта (дифференциация Container Limit OOM от системного исчерпания памяти ноды Node OOM).
* **FinOps Tetris & OOM Hazard Map:** визуализация срезов емкости нод по схемам `Allocatable` ↔ `Requests` ↔ `Limits` и расчет риска каскадного падения ноды (`Hazard Ratio`).
* **Admission Webhook Inspector:** детекция скрытых точек отказа в управляющем контуре Kubernetes (блокировка создания ресурсов из-за комбинации `failurePolicy: Fail` и недоступных вебхук-бэкендов).
* **Реактивный UI без расхода ресурсов:** цикл отрисовки на базе `ratatui` и `crossterm` потребляет 0% CPU в простое.

---

## Архитектура системы

```text
               +-------------------------------------------+
               |           Kubernetes API-Server           |
               +---------------------+---------------------+
                                     |
               (Параллельный опрос через tokio::try_join!)
                                     v
+------------------------------------+------------------------------------+
|                      Cluster Snapshot Cache                             |
|  - Gateways / VirtualServices / DestinationRules / ServiceEntries      |
|  - Core V1 Services, Selectors & Endpoints                              |
|  - Pods, ContainerStatuses & Node Allocatable Capacities               |
|  - Cluster Events & Admission Webhook Configurations                   |
+------------------+------------------+------------------+----------------+
                   |                  |                  |
                   v                  v                  v
+------------------+--+ +-------------+---+ +------------+-----+
|   Mesh Analyzer     | | Hop Flow Engine | | Kube-Forensics   |
| - Duplicates O(N)   | | - Gateway       | | - Incident Time- |
| - Broken/Dead Ends  | | - VirtualService| |   line & Verdict |
| - ServiceEntry Sync | | - K8s Service   | | - FinOps Tetris  |
|                     | | - Pod Matching  | | - Webhook Traps  |
+------------------+--+ +-------------+---+ +------------+-----+
                   \                  |                  /
                    \                 |                 /
                     v                v                v
                   +------------------------------------+
                   |      AppState (Central Bus)        |
                   +------------------+-----------------+
                                      |
                     (Реактивная перерисовка по событиям)
                                      v
                   +------------------------------------+
                   |    Ratatui TUI Rendering Engine    |
                   +------------------------------------+
```

---

## Функциональные модули

### [1] Дубликаты (Duplicates)
Анализирует пересечения правил в конфигурациях Istio.
* **DestinationRule Conflicts:** обнаруживает несколько манифестов `DestinationRule`, сконфигурированных на один и тот же целевой `host`. Предотвращает недетерминированное поведение Envoy (перезапись настроек mTLS, алгоритмов балансировки и outlier detection).
* **VirtualService Route Collision:** находит пересекающиеся манифесты `VirtualService`, зарегистрированные на одном и том же шлюзе (`gateways`) и слушающие одинаковые хосты (`hosts`).

### [2] Неиспользуемые ресурсы (Orphans & Dead Ends)
Выявляет «висячие» ресурсы и тупиковые маршруты.
* **Orphaned Gateways:** шлюзы, на которые не ссылается ни один активный `VirtualService`.
* **Broken Target Service:** правила `VirtualService`, перенаправляющие трафик на имена сервисов, которых нет в `v1/Service` и которые не объявлены в `ServiceEntry`.
* **Dead Ends (Zero Ready Pods):** маршруты, которые формально ведут на существующий Kubernetes Service, но у сервиса насчитывается ровно 0 готовых подов (все поды либо отсутствуют, либо находятся в фазах `CrashLoopBackOff`, `ImagePullBackOff`, `Pending`).

### [3] Граф трафика (Traffic Hop-by-Hop Canvas)
Разделен на две интерактивные панели:
* **Слева (32%):** список всех обнаруженных маршрутов `Namespace/Name [URI Match]` с поддержкой вертикального скролла через `ListState`.
* **Справа (68%):** декомпозиция цепочки передачи пакета:
  1. `1. GATEWAY`: сетевой периметр, порты и разрешенные входящие хосты.
  2. `2. VIRTUAL SERVICE`: условия сопоставления (`Exact`, `Prefix`, `Regex`) и целевой хост.
  3. `3. K8S SERVICE & ROUTE TARGET`: селекторы сервиса, открытые порты, примененный `DestinationRule`, имя сабсета и установленный вес балансировки (Traffic Weight %).
  4. `4. APPLICATION PODS & WORKLOADS`: список реальных подов приложения с расчетом готовности контейнеров `[Ready 1/1]` / `[Not Ready]` и их текущих фаз.

```text
┌── 🌐 1. GATEWAY ────────────────────────────────────────────────────────┐
│  Name:  istio-ingressgateway                                            │
│  Hosts: api.corp.internal, checkout.corp.internal                       │
└───┬────────────────────────────────────────────────────────────────────┘
    │  (Ingress Routing Rule)
    ▼
┌── 🔀 2. VIRTUAL SERVICE ───────────────────────────────────────────────┐
│  Name:  prod/checkout-vs                                                │
│  Match: Prefix(/api/v2) ──► Target: checkout-service.prod.svc           │
└───┬────────────────────────────────────────────────────────────────────┘
    │  (Forward to K8s Service)
    ▼
┌── ⚙️  3. K8S SERVICE & ROUTE TARGET ─────────────────────────────────────┐
│  Host:     checkout-service | Ports: http:8080                          │
│  Selector: app=checkout, tier=backend                                   │
│  Policy:   DestinationRule: checkout-dr | Subset: 'v2' | Weight: 100%   │
└───┬────────────────────────────────────────────────────────────────────┘
    │  (Endpoint Label Selection: app=checkout, tier=backend, version=v2)
    ▼
┌── 📦 4. APPLICATION PODS & WORKLOADS ───────────────────────────────────┐
│  Status: ● Все реплики здоровы (Ready: 2/2)                             │
│  • Pod: checkout-v2-7f98b6c4-x12ab     Phase: Running   [Ready 1/1]     │
│  • Pod: checkout-v2-7f98b6c4-y34cd     Phase: Running   [Ready 1/1]     │
└────────────────────────────────────────────────────────────────────────┘
```

### [4] Causal Incident Timeline
«Черный ящик» для разбора причин аварийных перезапусков подов.
* Коррелирует события пода (Kubelet, Container Runtime) с событиями ноды, на которой он выполнялся.
* Анализирует код завершения (`Exit Code`) и причину `last_state.terminated`.
* **Автоматический вердикт первопричины:**
  * **Node OOM-Killer:** сопоставляет код `137 (OOMKilled)` с событиями `NodeHasMemoryPressure` на ноде. Сигнализирует о нехватке памяти на хосте в целом, а не о нарушении индивидуальных лимитов пода.
  * **Container Limit OOM:** падение процесса по лимиту памяти контейнера при спокойной ноде.
  * **Liveness Probe Timeout:** принудительный SIGKILL со стороны Kubelet.
  * **Preemption / Eviction:** вытеснение пода Kubelet'ом в пользу более приоритетных воркстейтов.

### [5] FinOps Tetris & OOM Hazard Map
Контроль утилизации и рисков переподписки нод.
* Анализирует емкость нод `Allocatable` и строит графический индикатор распределения ресурсов:
  `[████████▓▓▓▓░░░░░░]`
  * `█` — Гарантированный резерв по `Requests` (базовый плацдарм, учитываемый K8s Scheduler).
  * `▓` — Воздушная подушка `Limits` (зона переподписки / Overcommit).
  * `░` — Неразмеченная физическая емкость ноды.
* **OOM Hazard Ratio:** коэффициент риска взрыва ноды:
  `Hazard Ratio = Sum(Memory Limits) / Node Allocatable Memory`
  * `> 2.0x` — **Критический риск**: при одновременном всплеске нагрузки поды попытаются утилизировать свои лимиты, что приведет к вызову системного OOM-Killer ядра Linux и непредсказуемому уничтожению процессов.
* Классифицирует поды по уровням качества обслуживания (QoS): `Guaranteed`, `Burstable`, `BestEffort`.

### [6] Admission Webhooks Trap Inspector
Анализирует цепочки мутации и валидации API-сервера (`MutatingWebhookConfiguration`, `ValidatingWebhookConfiguration`).
* Находит вебхуки, перехватывающие операцию `CREATE` для ресурса `pods`.
* Вычисляет критические условия блокировки API:
  `failurePolicy == "Fail" && Endpoints(Webhook Target Service) == 0`
  Если бэкенд вебхука недоступен или упал, API-сервер блокирует создание любых подов в кластере. Утилита маркирует такие правила статусом **`BLOCKER`**.

---

## Навигация и горячие клавиши

| Клавиша | Контекст | Действие |
| :--- | :--- | :--- |
| `1` – `6` | Глобальный | Быстрый переход между вкладками [1]–[6] |
| `Tab` / `BackTab` | Глобальный | Циклическое переключение вкладок вперед / назад |
| `q` / `Esc` | Глобальный | Закрытие модального окна / выход из программы |
| `/` | Вкладки 1, 2 | Полнотекстовый поиск с фильтрацией на лету |
| `n` | Глобальный | Выбор Scope пространства имен (Namespace) |
| `Enter` | Вкладки 1, 2 | Открытие модального инспектора манифеста с полным YAML |
| `j` / `↓` | Вкладки 1, 2, 4, 5, 6 | Перемещение курсора вниз по списку/таблице |
| `k` / `↑` | Вкладки 1, 2, 4, 5, 6 | Перемещение курсора вверх по списку/таблице |
| `j` / `↓` | Вкладка 3 (Граф) | Выбор следующего маршрута трафика |
| `k` / `↑` | Вкладка 3 (Граф) | Выбор предыдущего маршрута трафика |
| `d` | Вкладка 3 (Граф) | Прокрутка детальной карточки маршрута (правой панели) вниз |
| `u` | Вкладка 3 (Граф) | Прокрутка детальной карточки маршрута (правой панели) вверх |

---

## Настройка RBAC

Для полноценной работы утилите требуются права только на чтение (`get`, `list`) системных ресурсов и сетевых CRD:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: istio-lens-reader
rules:
  # Istio Service Mesh CRDs
  - apiGroups: ["networking.istio.io"]
    resources:
      - gateways
      - virtualservices
      - destinationrules
      - serviceentries
    verbs: ["get", "list"]

  # Kubernetes Core Resources
  - apiGroups: [""]
    resources:
      - services
      - pods
      - nodes
      - events
      - endpoints
    verbs: ["get", "list"]

  # Admission Registration
  - apiGroups: ["admissionregistration.k8s.io"]
    resources:
      - mutatingwebhookconfigurations
      - validatingwebhookconfigurations
    verbs: ["get", "list"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRoleBinding
metadata:
  name: istio-lens-reader-binding
subjects:
  - kind: User
    name: system:authenticated
    apiGroup: rbac.authorization.k8s.io
roleRef:
  kind: ClusterRole
  name: istio-lens-reader
  apiGroup: rbac.authorization.k8s.io
```

---

## Сборка и запуск

### 1. Локальная сборка (требуется Rust 1.75+)

```bash
cargo build --release
./target/release/istio-lens
```

### 2. Сборка статического бинарника под Linux (MUSL)

Создает полностью переносимый бинарник без внешних зависимостей от системного `glibc`:

```bash
rustup target add x86_64-unknown-linux-musl
sudo apt-get install -y musl-tools # Для Debian/Ubuntu

cargo build --release --target x86_64-unknown-linux-musl
```

### 3. Сборка через Docker (без установки локального тулчейна Rust)

Создайте `Dockerfile`:

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl && \
    rm -rf src

COPY src ./src
RUN touch src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl

FROM scratch AS export
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/istio-lens /istio-lens
```

Соберите и скопируйте бинарник в текущую директорию:

```bash
docker build --output type=local,dest=. -f Dockerfile .
chmod +x istio-lens
./istio-lens
```

---

## CI/CD: Автоматическая сборка релизов

Для автоматической публикации бинарников создайте файл `.github/workflows/build.yml`:

```yaml
name: Build & Release

on:
  push:
    branches: [ "main", "master" ]
    tags: [ "v*" ]
  workflow_dispatch:

permissions:
  contents: write

jobs:
  build:
    name: Build Linux MUSL Binary
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-unknown-linux-musl

      - name: Install musl-tools
        run: sudo apt-get update && sudo apt-get install -y musl-tools

      - name: Cache dependencies
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-musl-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-musl-

      - name: Build static binary
        run: cargo build --release --target x86_64-unknown-linux-musl

      - name: Prepare binary
        run: |
          mkdir -p dist
          cp target/x86_64-unknown-linux-musl/release/istio-lens dist/istio-lens-linux-amd64
          chmod +x dist/istio-lens-linux-amd64

      - name: Upload Artifact
        uses: actions/upload-artifact@v4
        with:
          name: istio-lens-linux-amd64
          path: dist/istio-lens-linux-amd64

      - name: Create GitHub Release
        if: startsWith(github.ref, 'refs/tags/v')
        uses: softprops/action-gh-release@v2
        with:
          files: dist/istio-lens-linux-amd64
          generate_release_notes: true
```

---

## Траблшутинг

* **`Error: Kube API request failed: ApiError: Forbidden`:** у текущей учетной записи отсутствуют права на чтение части кластерных ресурсов (например, `events` или `mutatingwebhookconfigurations`). Примените манифест RBAC или переключитесь на контекст с ролью `cluster-admin`.
* **Выбор альтернативного контекста Kubernetes:** утилита следует стандартному механизму резолвинга Kubeconfig. Для работы с другим кластером достаточно передать переменную окружения:
  ```bash
  KUBECONFIG=/path/to/stage-kubeconfig.yaml ./istio-lens
  ```
* **Сбой отображения терминала:** если процесс был принудительно завершен сигналом `SIGKILL` в обход встроенного `panic hook`, верните терминал в рабочий режим командой:
  ```bash
  reset
  ```
