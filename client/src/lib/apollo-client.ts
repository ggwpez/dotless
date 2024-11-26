import { ApolloClient, InMemoryCache } from '@apollo/client';

export const apolloClient = new ApolloClient({
  uri: 'https://dotburned.squids.live/polkadot-inflation-sqd@v1/api/graphql',
  cache: new InMemoryCache({
    typePolicies: {
      Query: {
        fields: {
          inflationData: {
            merge: true,
          },
        },
      },
    },
  }),
  defaultOptions: {
    watchQuery: {
      fetchPolicy: 'cache-and-network',
      nextFetchPolicy: 'cache-first',
    },
  },
});
