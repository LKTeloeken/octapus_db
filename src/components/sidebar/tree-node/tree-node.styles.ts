import { tw } from "@/lib/utils";

export const useStyles = () => ({
  root: tw`relative`,
  container: tw`flex items-center gap-2 py-1 px-2 cursor-pointer hover:text-foreground hover:bg-surface-light/50 rounded-md transition-color group relative pr-8`,
  iconWrapper: tw`flex items-center justify-center w-4 h-4 shrink-0`,
  spinner: tw`w-3 h-3`,
  chevronIcon: tw`w-4 h-4 text-foreground`,
  contentWrapper: tw`flex items-center gap-2`,
  textWrap: tw`flex flex-col gap-1`,
  nameText: tw`text-sm truncate`,
  dataTypeText: tw`text-xs text-foreground truncate`,
  menuButtonWrapper: tw`absolute right-2 transition-opacity`,
  menuButton: tw`h-6 w-6`,
  popoverContent: tw`w-32 p-1`,
  menuItemButton: tw`w-full justify-start gap-2`,
  menuIcon: tw`h-3 w-3`,
  verticalLine: tw`absolute top-0 w-0.5 bg-secondary`,
  horizontalLine: tw`absolute top-[1.125rem] h-0.5 bg-secondary`,
});
