import { HugeiconsIcon } from '@hugeicons/react';
import { PlayIcon, TextAlignLeftIcon } from '@hugeicons/core-free-icons';
import { memo, type FC } from 'react';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import type { QueryEditorToolbarProps } from './query-editor-toolbar.types';

export const QueryEditorToolbar: FC<QueryEditorToolbarProps> = memo(
  ({ onRun, onFormat, isLoading = false, disabled = false }) => {
    return (
      <div className="flex items-center gap-2 px-3 py-2 bg-background/50">
        <Tooltip content="Execute query (Ctrl+Enter)">
          <Button
            variant="default"
            size="sm"
            onClick={onRun}
            disabled={disabled || isLoading}
            className="gap-1.5"
          >
            {isLoading ? (
              <Spinner className="h-4 w-4" />
            ) : (
              <HugeiconsIcon icon={PlayIcon} className="h-4 w-4" />
            )}
            Run
          </Button>
        </Tooltip>

        <Tooltip content="Format SQL (Ctrl+Shift+F)">
          <Button
            variant="outline"
            size="sm"
            onClick={onFormat}
            disabled={disabled || isLoading}
            className="gap-1.5"
          >
            <HugeiconsIcon icon={TextAlignLeftIcon} className="h-4 w-4" />
            Format
          </Button>
        </Tooltip>
      </div>
    );
  },
);

QueryEditorToolbar.displayName = 'QueryEditorToolbar';
