-- Fixture multi-tenant: um schema por cliente, as mesmas ~150 tabelas em cada.
--
-- Cada tabela tem PK e uma coluna toastável, então gera 4 linhas em pg_class
-- (heap, índice da PK, TOAST e índice do TOAST) — o mesmo perfil de um banco
-- de produção, que é o que pesa na varredura do catálogo e no
-- pg_total_relation_size.
--
-- Drift proposital, para os testes de "shapes":
--   * todo tenant múltiplo de 100 não tem as 3 últimas tabelas (migração pendente);
--   * todo tenant múltiplo de 250 tem uma coluna a mais na primeira tabela.

CREATE OR REPLACE FUNCTION perf_table_name(t int) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
    SELECT (ARRAY[
        'customers', 'orders', 'invoices', 'payments', 'products',
        'categories', 'suppliers', 'shipments', 'addresses', 'contacts',
        'users', 'roles', 'permissions', 'sessions', 'notifications',
        'tickets', 'comments', 'attachments', 'tags', 'projects',
        'tasks', 'events', 'webhooks', 'settings', 'reports'
    ])[((t - 1) % 25) + 1]
    || (ARRAY['', '_items', '_history', '_audit', '_settings', '_links'])[((t - 1) / 25) + 1]
$$;

CREATE OR REPLACE PROCEDURE perf_seed_tenants(first_n int, last_n int, tables_per int)
LANGUAGE plpgsql AS $$
DECLARE
    schema_name text;
BEGIN
    FOR i IN first_n..last_n LOOP
        schema_name := format('tenant_%s', lpad(i::text, 5, '0'));

        -- Commit por schema: se o seed for interrompido, schema existente = completo
        IF EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = schema_name) THEN
            CONTINUE;
        END IF;

        EXECUTE format('CREATE SCHEMA %I', schema_name);

        FOR t IN 1..tables_per LOOP
            IF i % 100 = 0 AND t > tables_per - 3 THEN
                CONTINUE;
            END IF;

            EXECUTE format(
                'CREATE TABLE %I.%I (
                    id bigint PRIMARY KEY,
                    tenant_ref int NOT NULL,
                    name text,
                    payload jsonb,
                    created_at timestamptz NOT NULL DEFAULT now()
                )',
                schema_name, perf_table_name(t)
            );
        END LOOP;

        IF i % 250 = 0 THEN
            EXECUTE format('ALTER TABLE %I.%I ADD COLUMN legacy_code text',
                schema_name, perf_table_name(1));
        END IF;

        COMMIT;
    END LOOP;
END $$;

-- Tabelas compartilhadas no public e um schema com tabela particionada: o pai
-- tem relkind 'p' (hoje invisível na árvore) e as partições, 'r'.
CREATE OR REPLACE PROCEDURE perf_seed_shared(partitions int)
LANGUAGE plpgsql AS $$
BEGIN
    CREATE TABLE IF NOT EXISTS public.plans (id bigint PRIMARY KEY, name text);
    CREATE TABLE IF NOT EXISTS public.tenant_registry (id bigint PRIMARY KEY, schema_name text);
    CREATE TABLE IF NOT EXISTS public.schema_migrations (version text PRIMARY KEY);

    CREATE SCHEMA IF NOT EXISTS events;
    CREATE TABLE IF NOT EXISTS events.event_log (
        id bigint NOT NULL,
        happened_at date NOT NULL,
        body text
    ) PARTITION BY RANGE (happened_at);

    FOR p IN 0..partitions - 1 LOOP
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS events.%I PARTITION OF events.event_log
                FOR VALUES FROM (%L) TO (%L)',
            format('event_log_p%s', lpad(p::text, 4, '0')),
            (date '2000-01-01' + make_interval(months => p))::date,
            (date '2000-01-01' + make_interval(months => p + 1))::date
        );
    END LOOP;

    COMMIT;
END $$;
