import {sts, Block, Bytes, Option, Result, EventType, RuntimeCtx} from '../support'

export const eraPaid =  {
    name: 'Staking.EraPaid',
    /**
     * The era payout has been set; the first balance is the validator-payout; the second is
     * the remainder from the maximum amount of reward.
     */
    v2000000: new EventType(
        'Staking.EraPaid',
        sts.struct({
            eraIndex: sts.number(),
            validatorPayout: sts.bigint(),
            remainder: sts.bigint(),
        })
    ),
}
