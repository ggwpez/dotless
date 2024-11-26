import type { Express } from "express";
import { ApolloServer } from '@apollo/server';
import { expressMiddleware } from '@apollo/server/express4';
import { buildSchema } from 'graphql';

const schema = buildSchema(`
  type InflationData {
    year: Int!
    inflationRate: Float!
    totalIssuance: Float!
    stakingRate: Float!
  }

  type Query {
    inflationData: [InflationData!]!
  }
`);

const root = {
  inflationData: () => {
    // Mock data - replace with actual data source
    return Array.from({ length: 10 }, (_, i) => ({
      year: 2024 + i,
      inflationRate: 7.0 - (i * 0.3),
      totalIssuance: 1_200_000_000 + (i * 50_000_000),
      stakingRate: 55.0 + (i * 0.5),
    }));
  },
};

export async function registerRoutes(app: Express) {
  const server = new ApolloServer({
    schema,
    rootValue: root,
  });

  await server.start();

  app.use('/api/graphql', expressMiddleware(server));
}
