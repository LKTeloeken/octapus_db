import type { MockColumn, MockDatabase, MockTable } from './types';
import * as gen from './rows';

/** OIDs reais do catálogo do Postgres — o front usa só para exibir/depurar */
const OID = {
  bool: 16,
  int8: 20,
  int4: 23,
  text: 25,
  json: 114,
  float8: 701,
  varchar: 1043,
  date: 1082,
  timestamptz: 1184,
  numeric: 1700,
  uuid: 2950,
  jsonb: 3802,
  textArray: 1009,
  int4Array: 1007,
} as const;

// Açúcar para declarar coluna sem repetir os seis campos toda vez.
type ColumnOverrides = Partial<Omit<MockColumn, 'name' | 'gen'>>;

const column = (
  name: string,
  dataType: string,
  typeOid: number | null,
  cell: gen.CellGen,
  overrides: ColumnOverrides = {},
): MockColumn => ({
  name,
  dataType,
  typeOid,
  isNullable: true,
  defaultValue: null,
  isPrimaryKey: false,
  isForeignKey: false,
  gen: cell,
  ...overrides,
});

const pk = (name = 'id'): MockColumn =>
  column(name, 'integer', OID.int4, gen.serial(), {
    isNullable: false,
    defaultValue: `nextval('${name}_seq'::regclass)`,
    isPrimaryKey: true,
  });

const fkColumn = (
  name: string,
  maxId: number,
  overrides: ColumnOverrides = {},
): MockColumn =>
  column(name, 'integer', OID.int4, gen.fk(maxId), {
    isForeignKey: true,
    ...overrides,
  });

const USUARIOS = 5_000;
const PRODUTOS = 300;
const PEDIDOS = 12_000;

const usuarios: MockTable = {
  name: 'usuarios',
  schema: 'public',
  tableType: 'table',
  rowEstimate: USUARIOS,
  columns: [
    pk(),
    column('nome', 'text', OID.text, gen.fullName(), { isNullable: false }),
    column('email', 'character varying', OID.varchar, gen.email(), {
      isNullable: false,
    }),
    column(
      'telefone',
      'character varying',
      OID.varchar,
      gen.phone({ nullRate: 0.25 }),
    ),
    column('nascimento', 'date', OID.date, gen.date()),
    column('ativo', 'boolean', OID.bool, gen.bool(), {
      isNullable: false,
      defaultValue: 'true',
    }),
    column('saldo', 'numeric', OID.numeric, gen.decimal(-200, 9_800)),
    column(
      'plano',
      'text',
      OID.text,
      gen.pick(['free', 'pro', 'business', 'enterprise']),
    ),
    // O caminho de coluna array foi o último a ser corrigido — merece dado real.
    column(
      'tags',
      'text[]',
      OID.textArray,
      gen.textArray(
        ['vip', 'beta', 'newsletter', 'inadimplente', 'parceiro'],
        0,
        3,
        {
          nullRate: 0.15,
        },
      ),
    ),
    column('preferencias', 'jsonb', OID.jsonb, gen.json({ nullRate: 0.2 })),
    column('token', 'uuid', OID.uuid, gen.uuid()),
    column(
      'criado_em',
      'timestamp with time zone',
      OID.timestamptz,
      gen.timestamp(2023, 2026),
      {
        isNullable: false,
        defaultValue: 'now()',
      },
    ),
  ],
  indexes: [
    {
      name: 'usuarios_pkey',
      columns: ['id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'usuarios_email_key',
      columns: ['email'],
      isUnique: true,
      isPrimary: false,
      indexType: 'btree',
    },
    {
      name: 'idx_usuarios_plano',
      columns: ['plano', 'ativo'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

const enderecos: MockTable = {
  name: 'enderecos',
  schema: 'public',
  tableType: 'table',
  rowEstimate: 800,
  columns: [
    pk(),
    fkColumn('usuario_id', USUARIOS, { isNullable: false }),
    column('logradouro', 'text', OID.text, gen.street(), { isNullable: false }),
    column(
      'complemento',
      'text',
      OID.text,
      gen.pick(['Apto 42', 'Casa', 'Bloco B', 'Fundos'], { nullRate: 0.5 }),
    ),
    column('cidade', 'text', OID.text, gen.city(), { isNullable: false }),
    column('uf', 'character(2)', OID.varchar, gen.state(), {
      isNullable: false,
    }),
    column('cep', 'character varying', OID.varchar, gen.zip()),
    column('principal', 'boolean', OID.bool, gen.bool(), {
      defaultValue: 'false',
    }),
  ],
  indexes: [
    {
      name: 'enderecos_pkey',
      columns: ['id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'idx_enderecos_usuario',
      columns: ['usuario_id'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

const produtos: MockTable = {
  name: 'produtos',
  schema: 'public',
  tableType: 'table',
  rowEstimate: PRODUTOS,
  columns: [
    pk(),
    column('sku', 'character varying', OID.varchar, gen.sku(), {
      isNullable: false,
    }),
    column('nome', 'text', OID.text, gen.productName(), { isNullable: false }),
    column(
      'descricao',
      'text',
      OID.text,
      gen.sentence(6, 20, { nullRate: 0.3 }),
    ),
    column('preco', 'numeric', OID.numeric, gen.decimal(9.9, 4_999), {
      isNullable: false,
    }),
    column('estoque', 'integer', OID.int4, gen.int(0, 900), {
      isNullable: false,
      defaultValue: '0',
    }),
    column(
      'categorias',
      'text[]',
      OID.textArray,
      gen.textArray(
        ['casa', 'escritorio', 'eletronicos', 'moda', 'esporte'],
        1,
        3,
      ),
    ),
    column(
      'avaliacao',
      'double precision',
      OID.float8,
      gen.decimal(1, 5, 1, { nullRate: 0.2 }),
    ),
    column('publicado', 'boolean', OID.bool, gen.bool(), {
      defaultValue: 'false',
    }),
    column(
      'atualizado_em',
      'timestamp with time zone',
      OID.timestamptz,
      gen.timestamp(2024, 2026),
    ),
  ],
  indexes: [
    {
      name: 'produtos_pkey',
      columns: ['id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'produtos_sku_key',
      columns: ['sku'],
      isUnique: true,
      isPrimary: false,
      indexType: 'btree',
    },
    {
      name: 'idx_produtos_categorias',
      columns: ['categorias'],
      isUnique: false,
      isPrimary: false,
      indexType: 'gin',
    },
  ],
};

const pedidos: MockTable = {
  name: 'pedidos',
  schema: 'public',
  tableType: 'table',
  rowEstimate: PEDIDOS,
  columns: [
    pk(),
    fkColumn('usuario_id', USUARIOS, { isNullable: false }),
    column(
      'status',
      'text',
      OID.text,
      gen.pick(['pendente', 'pago', 'enviado', 'entregue', 'cancelado']),
      {
        isNullable: false,
        defaultValue: "'pendente'::text",
      },
    ),
    column('total', 'numeric', OID.numeric, gen.decimal(19.9, 12_000), {
      isNullable: false,
    }),
    column(
      'frete',
      'numeric',
      OID.numeric,
      gen.decimal(0, 120, 2, { nullRate: 0.1 }),
    ),
    column(
      'cupom',
      'text',
      OID.text,
      gen.pick(['BEMVINDO10', 'FRETEGRATIS', 'BLACK30'], { nullRate: 0.7 }),
    ),
    column(
      'itens_ids',
      'integer[]',
      OID.int4Array,
      gen.intArray(1, 20_000, 1, 5),
    ),
    column(
      'observacao',
      'text',
      OID.text,
      gen.sentence(3, 14, { nullRate: 0.6 }),
    ),
    column(
      'criado_em',
      'timestamp with time zone',
      OID.timestamptz,
      gen.timestamp(2024, 2026),
      {
        isNullable: false,
        defaultValue: 'now()',
      },
    ),
  ],
  indexes: [
    {
      name: 'pedidos_pkey',
      columns: ['id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'idx_pedidos_usuario',
      columns: ['usuario_id'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
    {
      name: 'idx_pedidos_status_data',
      columns: ['status', 'criado_em'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

/** View: sem PK → `editableInfo` null, o grid abre em modo leitura */
const vwPedidosResumo: MockTable = {
  name: 'vw_pedidos_resumo',
  schema: 'public',
  tableType: 'view',
  rowEstimate: 400,
  columns: [
    column(
      'mes',
      'text',
      OID.text,
      gen.pick([
        '2025-01',
        '2025-02',
        '2025-03',
        '2025-04',
        '2025-05',
        '2025-06',
      ]),
    ),
    column(
      'status',
      'text',
      OID.text,
      gen.pick(['pendente', 'pago', 'enviado', 'entregue', 'cancelado']),
    ),
    column('pedidos', 'bigint', OID.int8, gen.int(1, 900)),
    column('receita', 'numeric', OID.numeric, gen.decimal(100, 480_000)),
    column('ticket_medio', 'numeric', OID.numeric, gen.decimal(40, 1_200)),
  ],
  indexes: [],
};

const itensPedido: MockTable = {
  name: 'itens_pedido',
  schema: 'vendas',
  tableType: 'table',
  rowEstimate: 20_000,
  columns: [
    pk(),
    fkColumn('pedido_id', PEDIDOS, { isNullable: false }),
    fkColumn('produto_id', PRODUTOS, { isNullable: false }),
    column('quantidade', 'integer', OID.int4, gen.int(1, 12), {
      isNullable: false,
      defaultValue: '1',
    }),
    column('preco_unitario', 'numeric', OID.numeric, gen.decimal(9.9, 4_999), {
      isNullable: false,
    }),
    column(
      'desconto',
      'numeric',
      OID.numeric,
      gen.decimal(0, 200, 2, { nullRate: 0.4 }),
    ),
  ],
  indexes: [
    {
      name: 'itens_pedido_pkey',
      columns: ['id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'idx_itens_pedido',
      columns: ['pedido_id'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

const cupons: MockTable = {
  name: 'cupons',
  schema: 'vendas',
  tableType: 'table',
  rowEstimate: 120,
  columns: [
    column('codigo', 'text', OID.text, gen.sku(), {
      isNullable: false,
      isPrimaryKey: true,
    }),
    column('percentual', 'integer', OID.int4, gen.int(5, 60), {
      isNullable: false,
    }),
    column('validade', 'date', OID.date, gen.date(2025, 2027)),
    column('usos', 'integer', OID.int4, gen.int(0, 4_000), {
      defaultValue: '0',
    }),
    column('ativo', 'boolean', OID.bool, gen.bool(), { defaultValue: 'true' }),
  ],
  indexes: [
    {
      name: 'cupons_pkey',
      columns: ['codigo'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
  ],
};

const eventos: MockTable = {
  name: 'eventos',
  schema: 'public',
  tableType: 'table',
  rowEstimate: 8_000,
  columns: [
    column('id', 'bigint', OID.int8, gen.serial(), {
      isNullable: false,
      isPrimaryKey: true,
      defaultValue: "nextval('eventos_id_seq'::regclass)",
    }),
    column(
      'tipo',
      'text',
      OID.text,
      gen.pick(['page_view', 'add_to_cart', 'checkout', 'signup', 'error']),
    ),
    column('payload', 'jsonb', OID.jsonb, gen.json()),
    column('sessao', 'uuid', OID.uuid, gen.uuid()),
    column(
      'ocorrido_em',
      'timestamp with time zone',
      OID.timestamptz,
      gen.timestamp(2025, 2026),
    ),
  ],
  indexes: [
    {
      name: 'eventos_pkey',
      columns: ['id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'idx_eventos_tipo',
      columns: ['tipo', 'ocorrido_em'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

const metricasDiarias: MockTable = {
  name: 'metricas_diarias',
  schema: 'public',
  tableType: 'materializedview',
  rowEstimate: 900,
  columns: [
    column('dia', 'date', OID.date, gen.date(2024, 2026)),
    column('visitas', 'integer', OID.int4, gen.int(100, 90_000)),
    column('conversoes', 'integer', OID.int4, gen.int(0, 4_000)),
    column('receita', 'numeric', OID.numeric, gen.decimal(0, 320_000)),
  ],
  indexes: [],
};

/**
 * Estrutura completa do servidor Postgres do mock. Chamado também quando o
 * usuário cria um servidor postgres pelo formulário, para que o novo nó da
 * árvore também seja navegável.
 */
export function buildPostgresDatabases(): MockDatabase[] {
  return [
    {
      name: 'loja',
      sizeBytes: 412_509_696,
      tables: [
        usuarios,
        enderecos,
        produtos,
        pedidos,
        vwPedidosResumo,
        itensPedido,
        cupons,
      ].map(clone),
    },
    {
      name: 'analytics',
      sizeBytes: 1_284_308_992,
      tables: [eventos, metricasDiarias].map(clone),
    },
    { name: 'postgres', sizeBytes: 8_569_344, tables: [] },
  ];
}

/**
 * Cópia rasa com `rows` zerado: cada servidor precisa do seu próprio estado
 * mutável, senão editar numa conexão alteraria a outra.
 */
function clone(table: MockTable): MockTable {
  return { ...table, rows: undefined, nextSerial: undefined };
}
