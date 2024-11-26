import { Card } from "@/components/ui/card";
import ModelComparison from "../components/ModelComparison";

export default function HomePage() {
  return (
    <div className="min-h-screen bg-gradient-to-b from-black via-background to-black">
      <div className="container mx-auto p-4 space-y-6 relative">
        <div className="absolute inset-0 bg-[linear-gradient(45deg,rgba(230,0,122,0.1)_1px,transparent_1px),linear-gradient(-45deg,rgba(0,178,255,0.1)_1px,transparent_1px)] bg-[size:40px_40px] pointer-events-none" />
        <div className="space-y-2 relative">
          <h1 className="text-5xl font-bold tracking-tight bg-gradient-to-r from-neon-pink via-neon-purple to-neon-blue bg-clip-text text-transparent">
            Polkadot Inflation Model
          </h1>
          <p className="text-muted-foreground">
            Interactive visualization of Polkadot's economic model
          </p>
        </div>

        <p className="text-muted-foreground mb-4">
          This visualization compares Polkadot's old (10% fixed) and new inflation models, as implemented through{' '}
          <a 
            href="https://polkadot.subsquare.io/referenda/1139" 
            target="_blank" 
            rel="noopener noreferrer"
            className="text-primary hover:underline"
          >
            Referendum 1139
          </a>
          . The chart shows how much DOT has been saved by switching to the new model.
        </p>

        <Card className="p-6">
          <ModelComparison />
        </Card>
      </div>
    </div>
  );
}
