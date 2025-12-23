import { OperatorFunction, scan } from 'rxjs';

export function slidingWindow<T>(size: number): OperatorFunction<T, T[]> {
  return scan((acc: T[], value: T) => {
    acc.unshift(value);
    if (acc.length > size) {
      acc.pop();
    }
    return acc;
  }, [] as T[]);
}
