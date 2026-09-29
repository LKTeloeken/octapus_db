import { TableIcon } from '@hugeicons/core-free-icons';
import { ContentState } from '../ui/content-state';

interface EmptyStateProps {
  className?: string;
  message?: string;
}

export const EmptyState = ({
  className,
  message = 'Execute uma query para ver resultados',
}: EmptyStateProps) => {
  return <ContentState className={className} icon={TableIcon} title={message} />;
};
