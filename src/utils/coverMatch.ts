import COVER_MATCH_KEYWORDS from '../constants/cover-match-keywords.json'
import COVER_MATCH_ALIASES from '../constants/cover-match-aliases.json'

/**
 * 封面关键字匹配（支持别名）。
 *
 * 规则一「更长关键字优先（最具体优先）」：
 * 关键字之间存在包含关系（如「姜维」⊂「梦姜维」），
 * 短关键字的每次出现若都被更长的关键字覆盖，就说明它不是最具体的，需要剔除。
 * 这样标题「梦姜维」不会带出「姜维」封面，标题「姜维」也不会带出文件名含「梦姜维」的封面。
 *
 * 规则二「别名映射」：
 * 别名（如「悟空」→「孙桓」）会被映射回正式关键字，
 * 标题里出现别名时，等同于出现了正式关键字，可匹配到正式关键字的封面。
 */

// 参与匹配的文本项：term 为实际匹配的文本，canonical 为对应的正式关键字
interface CoverKeywordTerm {
    term: string
    canonical: string
    length: number
}

const COVER_KEYWORD_TERMS: CoverKeywordTerm[] = buildCoverKeywordTerms()

function buildCoverKeywordTerms(): CoverKeywordTerm[] {
    const terms: CoverKeywordTerm[] = []
    const added = new Set<string>()

    const addTerm = (term: string, canonical: string) => {
        const value = (term || '').trim()
        if (!value || added.has(value)) {
            return
        }
        added.add(value)
        terms.push({ term: value, canonical, length: value.length })
    }

    for (const keyword of COVER_MATCH_KEYWORDS) {
        addTerm(keyword, keyword)
    }

    // 别名映射到正式关键字，例如「悟空」→「孙桓」
    for (const [canonical, aliases] of Object.entries(COVER_MATCH_ALIASES)) {
        if (!COVER_MATCH_KEYWORDS.includes(canonical)) {
            console.warn(`[封面匹配] 别名配置中的「${canonical}」不是有效关键字，已忽略`)
            continue
        }
        const list: unknown[] = Array.isArray(aliases) ? aliases : [aliases]
        for (const alias of list) {
            if (typeof alias === 'string') {
                addTerm(alias, canonical)
            }
        }
    }

    return terms
}

// 文本中关键字出现的所有起始位置
function indexOfAll(text: string, keyword: string): number[] {
    const positions: number[] = []
    if (!keyword) {
        return positions
    }
    let pos = text.indexOf(keyword)
    while (pos !== -1) {
        positions.push(pos)
        pos = text.indexOf(keyword, pos + keyword.length)
    }
    return positions
}

// 文本中命中的「最具体」正式关键字（别名已映射回正式关键字）
export function matchCoverKeywords(text: string): string[] {
    if (!text) {
        return []
    }

    const hits = COVER_KEYWORD_TERMS.map(term => ({
        ...term,
        positions: indexOfAll(text, term.term)
    })).filter(hit => hit.positions.length > 0)

    const result: string[] = []
    for (const hit of hits) {
        // 每次出现都被其它关键字（更长）覆盖时，说明它不是最具体的（如「梦姜维」里的「姜维」）
        const allCovered = hit.positions.every(start => {
            const end = start + hit.length
            return hits.some(
                other =>
                    other.length > hit.length &&
                    other.canonical !== hit.canonical &&
                    other.positions.some(
                        otherStart => otherStart <= start && end <= otherStart + other.length
                    )
            )
        })
        if (allCovered) {
            continue
        }
        if (!result.includes(hit.canonical)) {
            result.push(hit.canonical)
        }
    }
    return result
}

// 图片是否匹配：文件名的最具体关键字需与标题命中的关键字有交集
export function isCoverImageMatched(name: string, matchedKeywords: string[]): boolean {
    const primary = matchCoverKeywords(name)
    return matchedKeywords.some(keyword => primary.includes(keyword))
}
