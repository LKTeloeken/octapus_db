import { memo, type FC } from 'react';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Play, AlignLeft } from 'lucide-react';

import type { QueryEditorToolbarProps } from './query-editor-toolbar.types';

export const QueryEditorToolbar: FC<QueryEditorToolbarProps> = memo(
  ({ onRun, onFormat, isLoading = false, disabled = false }) => {
    return (
      <div className="flex items-center gap-2 px-3 py-2 bg-background/50">
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
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
                  <Play className="h-4 w-4" />
                )}
                Run
              </Button>
            </TooltipTrigger>
            <TooltipContent>Execute query (Ctrl+Enter)</TooltipContent>
          </Tooltip>
        </TooltipProvider>

        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="outline"
                size="sm"
                onClick={onFormat}
                disabled={disabled || isLoading}
                className="gap-1.5"
              >
                <AlignLeft className="h-4 w-4" />
                Format
              </Button>
            </TooltipTrigger>
            <TooltipContent>Format SQL (Ctrl+Shift+F)</TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>
    );
  },
);

QueryEditorToolbar.displayName = 'QueryEditorToolbar';
