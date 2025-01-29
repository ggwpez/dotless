import { useQuery } from "@apollo/client";
import { Card } from "./ui/card";
import { GET_ERA_PAID_EVENTS } from "../lib/graphql/queries";
import { EraPaidEvent } from "../lib/graphql/types";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ResponsiveContainer,
  AreaChart,
  Area,
} from "recharts";
import { format, differenceInDays, parseISO, addDays } from "date-fns";
import { Skeleton } from "./ui/skeleton";

const generateProjectionData = (
  historicalData: Array<{
    timestamp: string;
    currentInflation: number;
    currentIssuance: number;
    legacyInflation: number;
    legacyIssuance: number;
    tooltipDate: string;
    isProjected: boolean,
  }>,
  projectionDays: number = 365 * 5, // 10 years
  yearlyIssuance: number = 120_000_000 // 120M DOT per year
) => {
  const dailyIssuance = yearlyIssuance / 365.25;
  let projectedData = [...historicalData];

  for (let i = 1; i <= projectionDays; i++) {
    const prevDay = projectedData[projectedData.length - 1];
    const projectedDate = addDays(new Date(prevDay.timestamp), 1);

    // Current model (linear issuance)
    const newCurrentIssuance = prevDay.currentIssuance + dailyIssuance;
    const currentInflation = (dailyIssuance * 365.25 / newCurrentIssuance) * 100;

    // Legacy model (10% exponential)
    const legacyDailyIncrease = (prevDay.legacyIssuance * (0.1 / 365.25));
    const newLegacyIssuance = prevDay.legacyIssuance + legacyDailyIncrease;

    projectedData.push({
      timestamp: projectedDate.toISOString(),
      currentInflation,
      currentIssuance: newCurrentIssuance,
      legacyInflation: 10,
      legacyIssuance: newLegacyIssuance,
      tooltipDate: format(projectedDate, "MMM d, yyyy HH:mm"),
      isProjected: true,
    });
  }

  // Every 10th to make UI faster
  return [
    ...projectedData
      .filter((_, index) => index % 30 === 0)
  ];
};

export default function ModelComparison() {
  const { loading, error, data } = useQuery<{ eraPaids: EraPaidEvent[] }>(
    GET_ERA_PAID_EVENTS,
  );

  if (loading) return <Skeleton className="w-full h-[600px]" />;
  if (error) return <div>Error loading comparison data: {error.message}</div>;

  const startDate = data?.eraPaids[0]?.timestamp || "";

  const getDateFormat = (data: any) => {
    if (!data || data.length < 2) return "MMM d, yyyy";

    // Calculate average time difference between points
    const avgTimeDiff = data.reduce((sum: number, point: { timestamp: string }, i: number) => {
      if (i === 0) return sum;
      const diff = differenceInDays(
        new Date(point.timestamp),
        new Date(data[i - 1].timestamp)
      );
      return sum + diff;
    }, 0) / (data.length - 1);

    // Choose format based on average difference
    if (avgTimeDiff <= 7) return "MMM d"; // Within a week
    if (avgTimeDiff <= 31) return "MMM d"; // Within a month
    return "MMM yyyy"; // Months apart
  };

  var lastCurrentIssuance = Number(data?.eraPaids[0]?.totalIssuance) / 1e10;
  var lastTi = Number(data?.eraPaids[0]?.totalIssuance) / 1e10;
  // Cum difference between legacy and current
  var cumDifference = Number(0);

  const historicalData =
    data?.eraPaids.map((event) => {
      const daysSinceStart = differenceInDays(
        parseISO(event.timestamp),
        parseISO(startDate),
      );
      const currentDailyIncrease = Number(event.amountPaid) / 1e10;
      var currentIssuance = Number(event.totalIssuance) / 1e10; // Convert to DOT

      const legacyDailyIncrease =
        (Number(event.totalIssuance) * (0.1 / 365.25)) / 1e10;

      const dailyDifference = legacyDailyIncrease - currentDailyIncrease;
      cumDifference += dailyDifference;

      const legacyIssuance =
        currentIssuance + legacyDailyIncrease + cumDifference;

      const currentInflation =
        (currentDailyIncrease / currentIssuance) * 365.25 * 100;
      currentIssuance += currentDailyIncrease;

      const legacyInflation = 10; // 10% fixed rate

      return {
        timestamp: event.timestamp,
        currentInflation,
        currentIssuance,
        legacyInflation,
        legacyIssuance,
        tooltipDate: format(new Date(event.timestamp), "MMM d, yyyy HH:mm"),
        isProjected: false,
      };
    }) || [];

  // Combine historical and projection data
  const chartData = generateProjectionData(historicalData);

  // Calculate saved amount using only historical data
  const latestHistoricalData = historicalData[historicalData.length - 1];
  const savedAmount = latestHistoricalData
    ? latestHistoricalData.legacyIssuance - latestHistoricalData.currentIssuance
    : 0;
  const latestCurrentIssuance = latestHistoricalData?.currentIssuance || 1; // Prevent division by zero

  const minIssuance = Math.min(
    ...chartData.map((d) => Math.min(d.currentIssuance, d.legacyIssuance)),
  );
  const maxIssuance = Math.max(
    ...chartData.map((d) => Math.max(d.currentIssuance, d.legacyIssuance)),
  );

  const formatNumber = (value: number, dec: number = 3) => {
    if (value > 1_000_000_000) {
      return `${(value / 1_000_000_000).toFixed(dec)}B`;
    } else if (value > 1_000_000) {
      return `${(value / 1_000_000).toFixed(dec)}M`;
    }
    return value.toFixed(dec);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <h2 className="text-2xl font-bold">Model Comparison over the next 5 years</h2>
      </div>

      <Card className="p-4 bg-black/80 border border-neon-pink/50 shadow-[0_0_15px_rgba(230,0,122,0.3)] relative overflow-hidden group transition-all duration-300 hover:shadow-[0_0_30px_rgba(230,0,122,0.5)]">
        <div className="absolute inset-0 bg-[linear-gradient(45deg,rgba(230,0,122,0.03)_1px,transparent_1px),linear-gradient(-45deg,rgba(0,178,255,0.03)_1px,transparent_1px)] bg-[size:20px_20px] pointer-events-none transition-opacity duration-300 group-hover:opacity-75" />
        <div className="absolute inset-0 bg-gradient-to-br from-neon-purple/10 via-transparent to-neon-blue/10 pointer-events-none" />
        <ResponsiveContainer width="100%" height={600}>
          <LineChart data={chartData}>
            <defs>
              <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
                <feGaussianBlur stdDeviation="2" result="coloredBlur" />
                <feMerge>
                  <feMergeNode in="coloredBlur" />
                  <feMergeNode in="SourceGraphic" />
                </feMerge>
              </filter>
            </defs>
            <CartesianGrid
              strokeDasharray="3 3"
              stroke="rgba(230,0,122,0.15)"
              className="transition-opacity duration-300 group-hover:opacity-75"
            />
            <XAxis
              dataKey="timestamp"
              stroke="#ffffff"
              tick={{ fill: "#ffffff" }}
              tickFormatter={(value) => format(new Date(value), "MMM yyyy")}
            />
            <YAxis
              yAxisId="rate"
              stroke="#ffffff"
              tick={{ fill: "#ffffff" }}
              label={{
                value: "Inflation (%)",
                angle: -90,
                position: "insideLeft",
                fill: "#ffffff",
              }}
            />
            <YAxis
              yAxisId="issuance"
              orientation="right"
              stroke="#ffffff"
              tick={{ fill: "#ffffff" }}
              label={{
                value: "Supply (DOT)",
                angle: 90,
                position: "insideRight",
                fill: "#ffffff",
              }}
              tickFormatter={formatNumber}
              domain={[
                (minIssuance: number) => minIssuance * 0.9999, // Slightly lower than min
                (maxIssuance: number) => maxIssuance * 1.0001, // Slightly higher than max
              ]}
            />
            <Tooltip
              contentStyle={{
                backgroundColor: "rgba(0,0,0,0.9)",
                border: "1px solid #E6007A",
                borderRadius: "4px",
                boxShadow: "0 0 10px rgba(230,0,122,0.3)",
                color: "#ffffff",
              }}
              labelFormatter={(value) => format(new Date(value), "MMM d, yyyy HH:mm")}
              formatter={(value: number, name: string) => [
                name.includes("Supply") ? `${formatNumber(value)} DOT` : value.toFixed(3) + "%",
                name,
              ]}
            />
            <Legend />
            <Line
              yAxisId="rate"
              type="monotone"
              dataKey="currentInflation"
              name="Inflation"
              stroke="#2680FF"
              strokeWidth={2}
              dot={false}
              filter="url(#glow)"
              className="transition-all duration-300 hover:opacity-90"
            />
            <Line
              yAxisId="issuance"
              type="monotone"
              dataKey="currentIssuance"
              name="Supply"
              stroke="#26FF80"
              strokeWidth={2}
              dot={false}
              filter="url(#glow)"
              className="transition-all duration-300 hover:opacity-90"
            />
            <Line
              yAxisId="rate"
              type="monotone"
              dataKey="legacyInflation"
              name="Old Inflation"
              stroke="#FF8026"
              strokeWidth={2}
              strokeDasharray="2 2"
              dot={false}
            />
            <Line
              yAxisId="issuance"
              type="monotone"
              dataKey="legacyIssuance"
              name="Old Supply"
              stroke="#FF2670"
              strokeWidth={2}
              strokeDasharray="2 2"
              dot={false}
              filter="url(#glow)"
            />
            <Area
              yAxisId="issuance"
              type="monotone"
              dataKey={(data: {
                legacyIssuance: number;
                currentIssuance: number;
              }) => data.legacyIssuance - data.currentIssuance}
              name="Cumulative Saved"
              fill="#FF267020"
              stroke="none"
            />
          </LineChart>
        </ResponsiveContainer>
      </Card>

      <Card className="p-4 bg-black/50">
        <div className="space-y-2">
          <h3 className="text-xl font-semibold">Cumulative DOT Saved</h3>
          <p className="text-muted-foreground mb-2">
            Amount of DOT that would have been additionally minted under the old
            10% exponential inflation model, compared to the current linear
            model.
          </p>
          <p className="text-lg">
            Saved:{" "}
            <span className="text-primary font-bold">
              {formatNumber(savedAmount, 0)} DOT (
              {((savedAmount / latestCurrentIssuance) * 100).toFixed(3)}% of
              current supply)
            </span>
          </p>
        </div>
      </Card>
    </div>
  );
}
