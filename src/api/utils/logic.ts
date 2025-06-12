export function convertToCamelCase(
  obj: Record<string, any>
): Record<string, any> {
  const newObj: Record<string, any> = {};
  for (const key in obj) {
    if (obj.hasOwnProperty(key)) {
      const newKey = key.replace(/_([a-z])/g, (g) => g[1].toUpperCase());
      newObj[newKey] = obj[key];
    }
  }
  return newObj;
}
