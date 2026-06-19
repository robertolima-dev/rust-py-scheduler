# Roadmap — o que falta

Estado em 2026-06-19. Etapas 1–22 concluídas: core do `Scheduler`, `every()`,
`cron()`, decorator, `list_jobs()`, `remove_job()`, retry, captura de erro,
integrações FastAPI/Django/Celery, e CI rodando os testes. 29 testes Rust
(`cargo test --no-default-features --lib`) e 74 testes Python (`pytest`)
passando.

Só restam as etapas que **dependem de ação externa do usuário** (criar
contas/tokens, criar o repositório no GitHub) — descritas abaixo.

---

## ✅ Concluído nesta rodada

- **Etapa 17 — FastAPI:** `python/rust_py_scheduler/fastapi.py` com
  `scheduler_lifespan(scheduler, app_lifespan=None)`, usando o `lifespan`
  moderno (não a API `on_event` depreciada). Testes em `tests/test_fastapi.py`
  dirigindo o ciclo completo via `TestClient` como context manager.
- **Etapa 18 — Django:** `python/rust_py_scheduler/django.py` com
  `start_in_background(scheduler, register_atexit=True)`. Start via
  `AppConfig.ready()`, idempotente por processo (guarda os schedulers já
  iniciados num `set`, não por `id()` — `id()` é reusado após GC), e shutdown
  best-effort via `atexit`. Documentado o comportamento com múltiplos workers
  (um scheduler por processo, sem coordenação). Testes em `tests/test_django.py`
  com `settings.configure()` mínimo.
- **Etapa 19 — Celery:** decidido que é só padrão + exemplo, sem módulo novo
  (`.delay`/`.apply_async` já são callables). `examples/celery_example.py` e
  seção no README.
- **Etapa 20 — Cron:** `src/cron.rs` com parser de 5 campos
  (`minuto hora dia-do-mês mês dia-da-semana`), suporte a `*`, `a`, `a-b`,
  `*/passo`, listas, e a regra Vixie (OR quando dia-do-mês e dia-da-semana
  estão ambos restritos). `Schedule::Cron(CronSchedule)` ao lado de
  `Schedule::Every(Duration)`; `registry::schedule_next()` calcula a próxima
  ocorrência wall-clock (via `chrono::Local`) e converte o gap num `Instant`.
  `EveryDecorator` virou `JobDecorator` genérico sobre `Schedule`. Testes em
  `src/cron.rs` (Rust) e `tests/test_cron.py` (Python).
- **Etapa 22 — CI:** adicionado um job `test` no `.github/workflows/CI.yml`
  que roda `cargo test --no-default-features --lib` + `maturin develop` +
  `pytest` em todo push/PR. Os jobs de build de wheel e o `release` agora
  dependem de `test`, então um PR que quebra os testes não gera nem publica
  pacote.

**Decisão de fuso horário (cron):** expressões são avaliadas no fuso local do
sistema (`chrono::Local`). Cobrir fuso configurável ficou para depois.

---

## Etapa 23 — TestPyPI

- Criar conta no TestPyPI (https://test.pypi.org/), gerar um API token.
- Configurar o token como secret no repositório GitHub (nome sugerido:
  `TEST_PYPI_API_TOKEN`, separado do `PYPI_API_TOKEN` de produção já referenciado
  no workflow).
- Adicionar um job/workflow (ou um `workflow_dispatch` manual) que publique em
  `--repository testpypi` antes de ir para o PyPI real — útil para validar que
  o pacote instala e importa corretamente (`pip install --index-url
  https://test.pypi.org/simple/ rust-py-scheduler`) antes do release de
  verdade.
- **Depende de ação externa do usuário** (criar conta/token) — não é algo que
  eu consiga fazer de forma autônoma.

---

## Etapa 24 — PyPI

- Confirmar que o nome `rust-py-scheduler` está disponível no PyPI (checar em
  https://pypi.org/project/rust-py-scheduler/ — se já existir, vai precisar de
  um nome alternativo).
- Gerar o `PYPI_API_TOKEN` real de produção e configurar como secret no
  repositório (o workflow `CI.yml` já espera esse nome de secret).
- Confirmar a estratégia de versionamento: hoje `Cargo.toml` tem
  `version = "0.1.0"`, e `pyproject.toml` usa `dynamic = ["version"]` (puxa a
  versão do `Cargo.toml` via maturin) — então bump de versão é só editar o
  `Cargo.toml` antes de criar a tag.
- Criar a primeira tag (ex: `v0.1.0`) — isso dispara o job `release` já
  existente no workflow, que builda tudo e publica.
- Depende das Etapas 22 (CI rodando testes — ✅ feito) e 23 (validação no
  TestPyPI) estarem resolvidas primeiro, pra não publicar no PyPI real sem rede
  de segurança.

**Bloqueio comum a 22/23/24:** o projeto ainda **não é um repositório git**
(sem `.git`, sem remoto). GitHub Actions só roda em repositórios hospedados no
GitHub — então antes de validar a CI/release de fato, é preciso `git init` +
primeiro commit, criar o repositório no GitHub e dar `git push`. Isso cabe ao
usuário (envolve uma conta/repositório externo).

---

## Decisões já tomadas (pra não re-discutir do zero)

- `max_retries` conta por **tick** (ciclo de agendamento), não por tentativa
  individual: só incrementa `error_count` depois de esgotar todas as
  tentativas; qualquer sucesso dentro do ciclo zera `last_error`.
- `shutdown()` é **mão única** — não dá para reiniciar um `Scheduler` depois
  de parado; o padrão de uso é "start no startup da app, shutdown no
  teardown".
- `list_jobs()` retorna `list[dict]`, não uma `pyclass` dedicada — escolha
  deliberada por simplicidade/inspeção fácil do lado Python.
- Cron usa resolução de minuto e fuso local; sub-minuto continua sendo
  `every("30s", ...)`.
- Build usa `abi3-py310`: um único wheel cobre Python 3.10 até 3.13+, sem
  precisar compilar por versão.
- Regra de mentoria a manter: avançar etapa por etapa, sempre explicar os
  mecanismos de PyO3/ownership/GIL quando aparecerem, sempre escrever teste
  antes de declarar uma etapa concluída, sempre rodar
  `cargo test --no-default-features --lib` + `maturin develop` + `pytest`
  antes de seguir.
