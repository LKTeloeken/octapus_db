export const useBuildQueries = () => {
  const quoteIdentifier = (value: string) =>
    `"${value.replace(/"/g, '""')}"`;

  const selectQuery = (
    schema: string,
    table: string,
    orderColumns: string[] = [],
    orderDirection: 'asc' | 'desc' = 'desc',
  ) => {
    const baseSelect = `SELECT * FROM ${quoteIdentifier(schema)}.${quoteIdentifier(table)}`;

    if (orderColumns.length === 0) return baseSelect;

    const orderBy = orderColumns
      .map(column => `${quoteIdentifier(column)} ${orderDirection.toUpperCase()}`)
      .join(', ');

    return `${baseSelect} ORDER BY ${orderBy}`;
  };

  return { selectQuery };
};
