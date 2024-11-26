type ModelType = "current" | "legacy";

export function calculateInflation(days: number, model: ModelType): number {
  switch (model) {
    case "current":
      // Use a simplified model for current inflation
      const annualRate = 7.0;
      const dailyRate = annualRate / 365.25;
      return Math.pow(1 + dailyRate / 100, days) * 100 - 100;
    case "legacy":
      // Calculate exponential inflation: daily rate compounded
      const legacyDailyRate = 10 / 365.25; // 10% annual rate
      return Math.pow(1 + legacyDailyRate / 100, days) * 100 - 100;
    default:
      return 0;
  }
}
