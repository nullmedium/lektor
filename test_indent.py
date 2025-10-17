# Test file for indentation
# Select multiple lines and press Tab to indent
# Press Shift+Tab to unindent

def main():
    print("Hello, World!")

    # Select these lines with Shift+Down
    # Then press Tab to indent them
    for i in range(10):
        print(f"Number: {i}")
        if i % 2 == 0:
            print("Even")
        else:
            print("Odd")

    # Test nested indentation
    data = {
        'name': 'Test',
        'items': [
            {'id': 1, 'value': 'first'},
            {'id': 2, 'value': 'second'},
        ]
    }

    # Select this block and indent/unindent
    if True:
        print("This is true")
        if False:
            print("This won't print")
        else:
            print("This will print")

if __name__ == "__main__":
    main()
