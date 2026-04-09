import { cn } from '@/lib/utils';
import { Typography } from '../ui/typography';
import { Spinner } from '../ui/spinner';

interface LodingStateProps {
  className?: string;
}

export const LodingState = ({ className }: LodingStateProps) => {
  return (
    <div className={cn('flex flex-col h-full', className)}>
      <div className="flex h-full items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <Spinner className="h-8 w-8" />
          <Typography variant="p" className="text-muted-foreground">
            Executando query...
          </Typography>
        </div>
      </div>
    </div>
  );
};
