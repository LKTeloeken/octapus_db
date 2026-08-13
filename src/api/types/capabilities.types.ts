export interface AdapterCapabilities {
  /** Postgres true; Mongo/Redis false — hide the schema level when false */
  hasSchemas: boolean;
  hasPrimaryKeys: boolean;
  /** Postgres only — drives editor highlight/placeholder */
  supportsSql: boolean;
  supportsTransactions: boolean;
  supportsIndexes: boolean;
  browsable: boolean;
}
