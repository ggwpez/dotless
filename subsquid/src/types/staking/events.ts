import {sts, Block, Bytes, Option, Result, EventType, RuntimeCtx} from '../support'
import * as v9090 from '../v9090'

export const eraPaid =  {
    name: 'Staking.EraPaid',
    /**
     *  The era payout has been set; the first balance is the validator-payout; the second is
     *  the remainder from the maximum amount of reward.
     *  \[era_index, validator_payout, remainder\]
     */
    v9090: new EventType(
        'Staking.EraPaid',
        sts.tuple([v9090.EraIndex, v9090.Balance, v9090.Balance])
    ),
    /**
     * The era payout has been set; the first balance is the validator-payout; the second is
     * the remainder from the maximum amount of reward.
     */
    v9300: new EventType(
        'Staking.EraPaid',
        sts.struct({
            eraIndex: sts.number(),
            validatorPayout: sts.bigint(),
            remainder: sts.bigint(),
        })
    ),
}
