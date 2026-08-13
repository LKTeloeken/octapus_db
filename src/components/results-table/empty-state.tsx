import { cn } from '@/lib/utils';
import { Typography } from '../ui/typography';

interface EmptyStateProps {
  className?: string;
  message?: string;
}

export const EmptyState = ({
  className,
  message = 'Execute uma query para ver resultados',
}: EmptyStateProps) => {
  return (
    <div className={cn('flex flex-col h-full', className)}>
      <div className="flex h-full items-center justify-center">
        <Typography variant="p" className="text-muted-foreground">
          {message}
        </Typography>
      </div>
    </div>
  );
};
