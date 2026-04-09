export const useBuildQueries = () => {
  const selectQuery = (
    schema: string,
    table: string,
    orderColumns: string[] = [],
    orderDirection: 'asc' | 'desc' = 'asc',
  ) => {
    const baseSelect = `SELECT * FROM ${schema}.${table}`;

    if (orderColumns.length === 0) return baseSelect;

    const orderBy = orderColumns
      .map(column => `${column} ${orderDirection}`)
      .join(', ');

    return `${baseSelect} ORDER BY ${orderBy} ${orderDirection}`;
  };

  return { selectQuery };
};
