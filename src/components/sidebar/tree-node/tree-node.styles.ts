import { tw } from '@/lib/utils';

export const useStyles = () => ({
  container: tw`flex items-center gap-2 py-1 px-2 cursor-pointer hover:text-foreground hover:bg-surface-light/50 rounded-md transition-color group relative`,
  iconWrapper: tw`flex items-center justify-center w-4 h-4 shrink-0`,
  spinner: tw`w-3 h-3`,
  chevronIcon: tw`w-4 h-4 text-foreground`,
  contentWrapper: tw`flex items-center gap-2 min-w-0 flex-1`,
  nodeIconWrapper: tw`w-4 h-4 min-w-4 min-h-4 flex items-center justify-center`,
  textWrap: tw`flex items-baseline gap-1.5 min-w-0`,
  nameText: tw`text-sm truncate`,
  dataTypeText: tw`text-xs text-muted-foreground truncate`,
  menuButtonWrapper: tw`flex items-center gap-1 transition-opacity`,
  menuButton: tw`h-6 w-6`,
});
