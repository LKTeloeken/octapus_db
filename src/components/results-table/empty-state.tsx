import { TabType } from '@/shared/models/tabs.types';
import { cn } from '@/lib/utils';
import { Typography } from '../ui/typography';

interface EmptyStateProps {
  className?: string;
  tabType?: TabType;
}

export const EmptyState = ({ className, tabType }: EmptyStateProps) => {
  return (
    <div className={cn('flex flex-col h-full', className)}>
      <div className="flex h-full items-center justify-center">
        <Typography variant="p" className="text-muted-foreground">
          {tabType === TabType.View
            ? 'Nenhum dado encontrado para visualização desta tabela'
            : 'Execute uma query para ver resultados'}
        </Typography>
      </div>
    </div>
  );
};
