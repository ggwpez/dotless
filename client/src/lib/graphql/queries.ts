import { gql } from '@apollo/client';

export const GET_ERA_PAID_EVENTS = gql`
  query GetEraPaidEvents {
    eraPaids(orderBy: timestamp_ASC) {
      id
      blockNumber
      timestamp
      amountPaid
      totalIssuance
    }
  }
`;
