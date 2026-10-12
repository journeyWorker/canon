public struct Cart {
    public private(set) var items: [String] = []
    public init() {}
    public mutating func add(_ item: String) { items.append(item) }
    public var count: Int { items.count }
}
