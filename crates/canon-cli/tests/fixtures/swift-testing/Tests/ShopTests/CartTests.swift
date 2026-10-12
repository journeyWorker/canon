import Testing
@testable import Shop

@Suite("Cart")
struct CartTests {
    @Test("shop.cart.01 adding an item counts it")
    func addCounts() {
        var cart = Cart()
        cart.add("apple")
        #expect(cart.count == 1)
    }

    @Test("shop.cart.02 an empty cart counts zero")
    func emptyCart() {
        #expect(Cart().count == 0)
    }

    @Test("shop.cart.03 adding twice keeps both", arguments: ["apple", "pear"])
    func addTwice(item: String) {
        var cart = Cart()
        cart.add(item)
        cart.add(item)
        #expect(cart.items == [item, item])
    }

    @Test func shop_cart_04_count_never_negative() {
        #expect(Cart().count >= 0)
    }

    @Test("shop.cart.05 a deliberately failing expectation")
    func failing() {
        #expect(Cart().count == 1)
    }

    @Test("shop.cart.06 not run yet", .disabled("pending design"))
    func skipped() {}
}
