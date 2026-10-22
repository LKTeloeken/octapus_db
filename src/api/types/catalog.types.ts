// Gerados do Rust (ts-rs) — ver src-tauri/src/models; não edite à mão
export type { CatalogDiagnostics } from './generated/CatalogDiagnostics';
export type { CatalogFolder } from './generated/CatalogFolder';
export type { CatalogObject } from './generated/CatalogObject';
export type { CatalogObjectRef } from './generated/CatalogObjectRef';
export type { CatalogEvent } from './generated/CatalogEvent';
export type { CatalogNode } from './generated/CatalogNode';
export type { CatalogPathInput as CatalogPath } from './generated/CatalogPathInput';
export type { CatalogSearchHit } from './generated/CatalogSearchHit';
export type { CatalogStats } from './generated/CatalogStats';
export type { CatalogStatus } from './generated/CatalogStatus';
export type { ColumnChange } from './generated/ColumnChange';
export type { ColumnSpec } from './generated/ColumnSpec';
export type { DriftGroup } from './generated/DriftGroup';
export type { DriftReport as CatalogDriftReport } from './generated/DriftReport';
export type { FunctionKind } from './generated/FunctionKind';
export type { FunctionObject } from './generated/FunctionObject';
export type { IndexObject as CatalogIndex } from './generated/IndexObject';
export type { LoadState as CatalogLoadState } from './generated/LoadState';
export type { NodeKind as CatalogNodeKind } from './generated/NodeKind';
export type { ObjectKind as CatalogObjectKind } from './generated/ObjectKind';
export type { Page as CatalogPage } from './generated/Page';
export type { Resolution as CatalogResolution } from './generated/Resolution';
export type { SchemaComparison } from './generated/SchemaComparison';
export type { SchemaDrift } from './generated/SchemaDrift';
export type { SchemaGroup } from './generated/SchemaGroup';
export type { ShapeGroup } from './generated/ShapeGroup';

/**
 * Contrato do catálogo de metadados (espelho de `src-tauri/src/models/catalog.rs`
 * e dos tipos de saída de `src-tauri/src/catalog/model.rs`). O backend guarda a
 * estrutura inteira; o front pede fatias: filhos de um nó, busca, resolução.
 */

/** Nome do evento global do Tauri com o progresso dos catálogos */
export const CATALOG_EVENT = 'catalog-event';

/**
 * Bancos sem nível de schema (Mongo, SQLite) guardam as relações num schema só,
 * sem nome: pede-se `{ kind: 'schema', schema: FLAT_SCHEMA }`, e na aba e na
 * palette ele vira "sem schema" (`null`).
 */
export const FLAT_SCHEMA = '';
