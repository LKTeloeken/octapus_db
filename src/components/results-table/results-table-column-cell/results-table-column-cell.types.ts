import type { ForeignKeyTarget, QueryColumnInfo } from '@/api/database/database-responses.types';
import type { TabSort } from '@/shared/models/tabs.types';

export interface ColumnCellProps {
  column: QueryColumnInfo;
  isPrimaryKeyColumn: boolean;
  className?: string;
  sortable?: boolean;
  sort?: TabSort | null;
  onSortColumn?: (column: string) => void;
  onOpenForeignTable?: (target: ForeignKeyTarget) => void;
}
