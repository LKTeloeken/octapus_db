import { cn } from '@/lib/utils';
import { Typography } from '../ui/typography';

interface EmptyStateProps {
  className?: string;
}

export const EmptyState = ({ className }: EmptyStateProps) => {
  return (
    <div className={cn('flex flex-col h-full', className)}>
      <div className="flex h-full items-center justify-center">
        <Typography variant="p" className="text-muted-foreground">
          Execute uma query para ver resultados
        </Typography>
      </div>
    </div>
  );
};
